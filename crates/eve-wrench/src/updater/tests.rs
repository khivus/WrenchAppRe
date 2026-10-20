use super::*;

fn fixture() -> PreparedUpdate {
    let parent = std::env::temp_dir().join(format!(
        "wrench-updater-test-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&parent).unwrap();
    let parent = parent.canonicalize().unwrap();
    let directory = parent.join(".eve-wrench-update-test");
    fs::create_dir(&directory).unwrap();
    let original = parent.join("EVE Wrench.exe");
    fs::write(&original, b"old executable").unwrap();
    fs::write(directory.join("new.exe"), b"new executable").unwrap();
    PreparedUpdate {
        original,
        directory,
        parent_pid: 0,
        sha256: String::new(),
        arguments: vec![],
        working_directory: parent,
    }
}

#[test]
fn replacement_preserves_original_on_relaunch_failure_and_keeps_same_filename() {
    for succeed in [false, true] {
        let prepared = fixture();
        prepared.validate().unwrap();
        let result = replace_and_launch(&prepared, || {
            assert_eq!(fs::read(&prepared.original).unwrap(), b"new executable");
            assert_eq!(
                fs::read(prepared.path("previous.exe")).unwrap(),
                b"old executable"
            );
            if succeed {
                Ok(())
            } else {
                Err("simulated restart failure".into())
            }
        });
        assert_eq!(result.is_ok(), succeed);
        assert_eq!(
            fs::read(&prepared.original).unwrap(),
            if succeed {
                b"new executable"
            } else {
                b"old executable"
            }
        );
        let parent = prepared.working_directory.clone();
        cleanup(&prepared);
        assert!(!prepared.directory.exists());
        assert!(prepared.original.exists());
        assert!(parent.starts_with(std::env::temp_dir().canonicalize().unwrap()));
        fs::remove_dir_all(parent).unwrap();
    }
}

#[test]
fn staging_scope_and_restart_arguments_are_checked() {
    assert_eq!(
        restart_arguments(
            [
                "--first",
                "--update-receipt",
                "stale.json",
                "space in argument"
            ]
            .into_iter()
            .map(String::from)
        ),
        ["--first", "space in argument"]
    );
    let mut prepared = fixture();
    prepared.directory = prepared.working_directory.clone();
    assert!(prepared.validate().is_err());
    // Invalid staging paths cannot be used to remove the executable folder.
    cleanup(&prepared);
    assert!(prepared.original.exists());
    assert!(
        prepared
            .working_directory
            .starts_with(std::env::temp_dir().canonicalize().unwrap())
    );
    fs::remove_dir_all(prepared.working_directory).unwrap();
}

#[cfg(windows)]
#[test]
fn helper_waits_for_a_process_to_exit_without_killing_it() {
    let mut command = Command::new("ping.exe");
    command
        .args(["-n", "2", "127.0.0.1"])
        .stdout(std::process::Stdio::null());
    hide_console(&mut command);
    let mut child = command.spawn().unwrap();
    let started = Instant::now();
    wait_for_parent(child.id()).unwrap();
    assert!(started.elapsed() >= Duration::from_millis(500));
    assert!(child.wait().unwrap().success());
}

#[cfg(windows)]
#[test]
fn full_helper_flow_replaces_a_fixture_and_restarts_it_at_the_original_path() {
    let mut prepared = fixture();
    let source = prepared.working_directory.join("fixture.rs");
    fs::write(
        &source,
        r#"
        #![windows_subsystem = "windows"]
        fn main() {
            let args: Vec<_> = std::env::args_os().collect();
            let index = args.iter().position(|arg| arg == "--update-receipt").unwrap();
            let path = std::path::Path::new(&args[index + 1]);
            std::fs::write(path.parent().unwrap().join("new.pid"), std::process::id().to_string()).unwrap();
            std::fs::write(path.parent().unwrap().join("started"), b"ready").unwrap();
        }
    "#,
    )
    .unwrap();
    let mut compiler = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()));
    compiler
        .arg("--crate-name")
        .arg("update_fixture")
        .arg(&source)
        .arg("-o")
        .arg(prepared.path("new.exe"));
    compiler.current_dir(&prepared.working_directory);
    hide_console(&mut compiler);
    let output = compiler.output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    // rustc emits a PDB beside the fixture; real downloads contain only the EXE.
    let _ = fs::remove_file(prepared.path("new.pdb"));
    // Use the actual verified-PE path; the hash comes from the freshly compiled fixture.
    let bytes = fs::read(prepared.path("new.exe")).unwrap();
    use sha2::{Digest, Sha256};
    prepared.sha256 = format!("{:x}", Sha256::digest(&bytes));
    // A surrogate old process exits normally; the real EVE Wrench is untouched.
    let mut old_process = Command::new("ping.exe");
    old_process
        .args(["-n", "2", "127.0.0.1"])
        .stdout(std::process::Stdio::null());
    hide_console(&mut old_process);
    let mut child = old_process.spawn().unwrap();
    prepared.parent_pid = child.id();
    fs::write(
        prepared.path("transaction.json"),
        serde_json::to_vec(&prepared).unwrap(),
    )
    .unwrap();
    apply(&prepared.path("transaction.json")).unwrap();
    assert!(child.wait().unwrap().success());
    assert_eq!(fs::read(&prepared.original).unwrap(), bytes);
    assert_eq!(
        fs::read(prepared.path("previous.exe")).unwrap(),
        b"old executable"
    );
    assert_eq!(
        fs::read_to_string(prepared.path("result.txt")).unwrap(),
        "ok"
    );
    assert!(prepared.path("started").exists());
    let pid = fs::read_to_string(prepared.path("new.pid"))
        .unwrap()
        .parse()
        .unwrap();
    wait_for_parent(pid).unwrap();
    fs::remove_file(prepared.path("new.pid")).unwrap();
    let parent = prepared.working_directory.clone();
    cleanup(&prepared);
    assert!(!prepared.directory.exists());
    assert!(parent.starts_with(std::env::temp_dir().canonicalize().unwrap()));
    fs::remove_dir_all(parent).unwrap();
}
