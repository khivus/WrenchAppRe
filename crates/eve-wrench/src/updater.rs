//! Portable replacement uses a copy of this executable as an unelevated helper.
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use eve_wrench_core::updates::{self, UpdateAsset};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct PreparedUpdate {
    original: PathBuf,
    directory: PathBuf,
    parent_pid: u32,
    sha256: String,
    arguments: Vec<String>,
    working_directory: PathBuf,
}

impl PreparedUpdate {
    fn path(&self, name: &str) -> PathBuf {
        self.directory.join(name)
    }

    fn validate(&self) -> Result<(), String> {
        let parent = self
            .original
            .parent()
            .ok_or("Executable has no parent folder")?;
        if self.directory.parent() != Some(parent)
            || !self
                .directory
                .file_name()
                .is_some_and(|name| name.to_string_lossy().starts_with(".eve-wrench-update-"))
            || !self.original.is_absolute()
            || !self.directory.is_absolute()
            || self.directory.canonicalize().map_err(|e| e.to_string())? != self.directory
            || parent.canonicalize().map_err(|e| e.to_string())? != parent
        {
            return Err("Invalid update staging directory".into());
        }
        Ok(())
    }
}

pub fn prepare(asset: &UpdateAsset) -> Result<PreparedUpdate, String> {
    let original = std::env::current_exe()
        .and_then(|path| path.canonicalize())
        .map_err(|e| e.to_string())?;
    let parent = original
        .parent()
        .ok_or("Cannot locate the application folder")?;
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let directory = parent.join(format!(".eve-wrench-update-{}-{nonce}", std::process::id()));
    fs::create_dir(&directory).map_err(|e| {
        format!(
            "Cannot update in this folder: {e}. Move EVE Wrench to a folder you own, then retry."
        )
    })?;
    let arguments = restart_arguments(std::env::args().skip(1));
    let prepared = PreparedUpdate {
        original,
        directory,
        parent_pid: std::process::id(),
        sha256: asset.sha256.clone(),
        arguments,
        working_directory: std::env::current_dir().map_err(|e| e.to_string())?,
    };
    let result = (|| {
        updates::download_update(asset, &prepared.path("new.exe"))?;
        fs::copy(&prepared.original, prepared.path("helper.exe")).map_err(|e| e.to_string())?;
        fs::write(
            prepared.path("transaction.json"),
            serde_json::to_vec(&prepared).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())
    })();
    if let Err(error) = result {
        cleanup(&prepared);
        return Err(error);
    }
    Ok(prepared)
}

fn restart_arguments(arguments: impl Iterator<Item = String>) -> Vec<String> {
    let mut result = Vec::new();
    let mut arguments = arguments;
    while let Some(arg) = arguments.next() {
        if arg == "--update-receipt" {
            arguments.next();
        } else {
            result.push(arg);
        }
    }
    result
}

pub fn launch(prepared: &PreparedUpdate) -> Result<(), String> {
    prepared.validate()?;
    for name in ["started", "result.txt"] {
        let path = prepared.path(name);
        if path.exists() {
            fs::remove_file(path).map_err(|e| e.to_string())?;
        }
    }
    let mut command = Command::new(prepared.path("helper.exe"));
    command
        .arg("--apply-update")
        .arg(prepared.path("transaction.json"));
    hide_console(&mut command);
    command
        .spawn()
        .map_err(|e| format!("Could not start the update helper: {e}"))?;
    Ok(())
}

#[cfg(windows)]
fn hide_console(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    command.creation_flags(0x08000000); // CREATE_NO_WINDOW; never request elevation.
}
#[cfg(not(windows))]
fn hide_console(_: &mut Command) {}

fn read_transaction(path: &Path) -> Result<PreparedUpdate, String> {
    let prepared: PreparedUpdate =
        serde_json::from_slice(&fs::read(path).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    prepared.validate()?;
    if path != prepared.path("transaction.json") {
        return Err("Invalid update transaction path".into());
    }
    Ok(prepared)
}

#[cfg(windows)]
fn wait_for_parent(pid: u32) -> Result<(), String> {
    use windows_sys::Win32::Foundation::{
        CloseHandle, ERROR_INVALID_PARAMETER, GetLastError, WAIT_OBJECT_0,
    };
    use windows_sys::Win32::System::Threading::{
        OpenProcess, PROCESS_SYNCHRONIZE, WaitForSingleObject,
    };
    // SYNCHRONIZE only observes the process; it cannot terminate or elevate it.
    unsafe {
        let handle = OpenProcess(PROCESS_SYNCHRONIZE, 0, pid);
        if handle.is_null() {
            return if GetLastError() == ERROR_INVALID_PARAMETER {
                Ok(())
            } else {
                Err("Cannot wait for the running application to close".into())
            };
        }
        let result = WaitForSingleObject(handle, 60_000);
        CloseHandle(handle);
        if result != WAIT_OBJECT_0 {
            return Err("Application did not close; update cancelled".into());
        }
    }
    Ok(())
}
#[cfg(not(windows))]
fn wait_for_parent(_: u32) -> Result<(), String> {
    Err("Portable self-update is available on Windows x64".into())
}

fn move_with_retry(from: &Path, to: &Path) -> Result<(), String> {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        match fs::rename(from, to) {
            Ok(()) => return Ok(()),
            Err(error) if Instant::now() >= deadline => return Err(error.to_string()),
            Err(_) => std::thread::sleep(Duration::from_millis(200)),
        }
    }
}

// Roll back the original if replacement or relaunch fails.
fn replace_and_launch(
    prepared: &PreparedUpdate,
    launch: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    move_with_retry(&prepared.original, &prepared.path("previous.exe"))?;
    if let Err(error) = move_with_retry(&prepared.path("new.exe"), &prepared.original) {
        move_with_retry(&prepared.path("previous.exe"), &prepared.original)?;
        return Err(format!(
            "Could not replace the executable; original restored: {error}"
        ));
    }
    if let Err(error) = launch() {
        move_with_retry(&prepared.original, &prepared.path("new.exe"))?;
        move_with_retry(&prepared.path("previous.exe"), &prepared.original)?;
        return Err(format!(
            "Could not restart the update; original restored: {error}"
        ));
    }
    Ok(())
}

fn start_application(prepared: &PreparedUpdate) -> Result<std::process::Child, String> {
    let mut command = Command::new(&prepared.original);
    command
        .args(&prepared.arguments)
        .arg("--update-receipt")
        .arg(prepared.path("transaction.json"));
    if prepared.working_directory.is_dir() {
        command.current_dir(&prepared.working_directory);
    }
    command.spawn().map_err(|e| e.to_string())
}

fn apply(path: &Path) -> Result<(), String> {
    let prepared = read_transaction(path)?;
    let mut parent_closed = false;
    let result = (|| {
        wait_for_parent(prepared.parent_pid)?;
        parent_closed = true;
        updates::verify_windows_exe(&prepared.path("new.exe"), &prepared.sha256)?;
        replace_and_launch(&prepared, || {
            let mut child = start_application(&prepared)?;
            let deadline = Instant::now() + Duration::from_secs(45);
            loop {
                if prepared.path("started").is_file() {
                    return Ok(());
                }
                if child.try_wait().map_err(|e| e.to_string())?.is_some() {
                    return Err("New application exited before opening its window".into());
                }
                if Instant::now() >= deadline {
                    // A slow but running app must not be killed to roll back.
                    return Ok(());
                }
                std::thread::sleep(Duration::from_millis(100));
            }
        })
    })();
    let status = match &result {
        Ok(()) => "ok".into(),
        Err(error) => error.clone(),
    };
    let _ = fs::write(prepared.path("result.txt"), status);
    if result.is_err()
        && parent_closed
        && !prepared.path("previous.exe").exists()
        && prepared.original.exists()
    {
        // Replacement never happened or was rolled back: reopen the old app.
        let _ = start_application(&prepared);
    }
    result
}

/// Handles helper mode before GPUI starts. Normal app startup returns false.
pub fn handle_helper_mode() -> bool {
    let mut args = std::env::args_os().skip(1);
    if args.next().as_deref() != Some(std::ffi::OsStr::new("--apply-update")) {
        return false;
    }
    if let Some(path) = args.next() {
        let _ = apply(Path::new(&path));
    }
    true
}

pub fn startup_error() -> Option<String> {
    let prepared = read_transaction(&receipt_path()?).ok()?;
    let result = fs::read_to_string(prepared.path("result.txt")).ok()?;
    (result != "ok").then_some(result)
}

fn receipt_path() -> Option<PathBuf> {
    let mut args = std::env::args_os().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--update-receipt" {
            return args.next().map(PathBuf::from);
        }
    }
    None
}

/// Acknowledge an opened window, then clean helper/staging files after it exits.
pub fn complete_startup() {
    let Some(prepared) = receipt_path().and_then(|path| read_transaction(&path).ok()) else {
        return;
    };
    let _ = fs::write(prepared.path("started"), b"ready");
    std::thread::spawn(move || {
        for _ in 0..240 {
            if prepared.path("result.txt").is_file()
                && fs::remove_file(prepared.path("helper.exe")).is_ok()
            {
                cleanup(&prepared);
                return;
            }
            std::thread::sleep(Duration::from_millis(250));
        }
    });
}

pub fn cleanup(prepared: &PreparedUpdate) {
    if prepared.validate().is_err() {
        return;
    }
    for name in [
        "new.exe",
        "previous.exe",
        "helper.exe",
        "transaction.json",
        "started",
        "result.txt",
    ] {
        let _ = fs::remove_file(prepared.path(name));
    }
    let _ = fs::remove_dir(&prepared.directory);
}

#[cfg(test)]
mod tests;
