use super::*;
use crate::{Locations, Server, scan};

struct Fixture {
    root: PathBuf,
    source: PathBuf,
    server: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "wrench-profile-copy-test-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&root).unwrap();
        let root = root.canonicalize().unwrap();
        let source = root.join("eve/c_source_tranquility/settings_Default");
        let server = root.join("eve/c_target_singularity");
        fs::create_dir_all(&source).unwrap();
        fs::create_dir_all(&server).unwrap();
        Self {
            root,
            source,
            server,
        }
    }
    fn populate(&self) {
        for (name, contents) in [
            ("core_user_1.dat", b"account bytes".as_slice()),
            ("core_char_2.dat", b"character bytes".as_slice()),
            (
                "core_public__.yaml",
                b"\xef\xbb\xbfdevice:\n  mode: 1\n".as_slice(),
            ),
            ("prefs.ini", b"bracketsAlwaysShowShipText=1\n".as_slice()),
            ("backups/earlier.bak", b"original backup bytes".as_slice()),
        ] {
            let path = self.source.join(name);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, contents).unwrap();
        }
        fs::create_dir_all(self.source.join("empty/nested")).unwrap();
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let root = self.root.canonicalize().unwrap();
        assert!(root.starts_with(std::env::temp_dir().canonicalize().unwrap()));
        assert!(
            root.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("wrench-profile-copy-test-")
        );
        fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn copies_all_files_and_named_profile_to_a_server_without_configurations() {
    let f = Fixture::new();
    f.populate();
    let locations = Locations::new(f.root.join("app"), Some(f.root.join("eve")));
    let before = scan::load_app_data(&locations).unwrap();
    assert!(
        before
            .servers
            .iter()
            .find(|s| s.info.id == Server::Singularity)
            .unwrap()
            .profiles
            .is_empty()
    );
    let result = copy_profile(&f.source, &f.server, "Named copy", true).unwrap();
    assert_eq!(
        (result.accounts, result.characters, result.files),
        (1, 1, 5)
    );
    assert!(result.backup.is_none());
    assert_eq!(result.destination, f.server.join("settings_Named copy"));
    for file in [
        "core_user_1.dat",
        "core_char_2.dat",
        "core_public__.yaml",
        "prefs.ini",
        "backups/earlier.bak",
    ] {
        assert_eq!(
            fs::read(result.destination.join(file)).unwrap(),
            fs::read(f.source.join(file)).unwrap()
        );
    }
    assert!(result.destination.join("empty/nested").is_dir());
    let after = scan::load_app_data(&locations).unwrap();
    let profile = &after
        .servers
        .iter()
        .find(|s| s.info.id == Server::Singularity)
        .unwrap()
        .profiles[0];
    assert_eq!(profile.name, "Named copy");
    assert_eq!((profile.accounts.len(), profile.characters.len()), (1, 1));
}

#[test]
fn replacement_preserves_a_complete_backup_and_other_profiles() {
    let f = Fixture::new();
    f.populate();
    let target = f.server.join("settings_Default");
    fs::create_dir_all(target.join("backups")).unwrap();
    fs::write(target.join("core_user_99.dat"), b"old account").unwrap();
    fs::write(target.join("core_public__.yaml"), b"old YAML").unwrap();
    fs::write(target.join("backups/old.bak"), b"old backup").unwrap();
    let other = f.server.join("settings_Other");
    fs::create_dir(&other).unwrap();
    fs::write(other.join("keep.txt"), b"other profile").unwrap();
    let result = copy_profile(&f.source, &f.server, "settings_Default", true).unwrap();
    assert!(!target.join("core_user_99.dat").exists());
    assert!(!target.join("backups/old.bak").exists());
    let backup = result.backup.unwrap();
    assert_eq!(
        fs::read(backup.join("core_user_99.dat")).unwrap(),
        b"old account"
    );
    assert_eq!(
        fs::read(backup.join("core_public__.yaml")).unwrap(),
        b"old YAML"
    );
    assert_eq!(
        fs::read(backup.join("backups/old.bak")).unwrap(),
        b"old backup"
    );
    assert_eq!(fs::read(other.join("keep.txt")).unwrap(), b"other profile");
    assert_eq!(
        fs::read(f.source.join("core_user_1.dat")).unwrap(),
        b"account bytes"
    );
    let locations = Locations::new(f.root.join("app"), Some(f.root.join("eve")));
    let data = scan::load_app_data(&locations).unwrap();
    let server = data
        .servers
        .iter()
        .find(|s| s.info.id == Server::Singularity)
        .unwrap();
    assert_eq!(
        server.profiles.len(),
        2,
        "Backup folders must not become EVE profiles"
    );
}

#[test]
fn profiles_with_only_public_yaml_and_no_dat_files_can_replace_without_backups() {
    let f = Fixture::new();
    fs::write(f.source.join("core_public__.yaml"), b"generic: {}\n").unwrap();
    let target = f.server.join("settings_New");
    fs::create_dir(&target).unwrap();
    fs::write(target.join("old.txt"), b"old").unwrap();
    let result = copy_profile(&f.source, &f.server, "New", false).unwrap();
    assert_eq!(
        (result.accounts, result.characters, result.files),
        (0, 0, 1)
    );
    assert!(result.backup.is_none());
    assert!(!target.join("old.txt").exists());
    assert_eq!(
        fs::read(target.join("core_public__.yaml")).unwrap(),
        b"generic: {}\n"
    );
    assert_eq!(
        fs::read_dir(&f.server).unwrap().count(),
        1,
        "Staging/old folders must be removed"
    );
}

#[test]
fn invalid_names_and_overlapping_folders_leave_the_source_untouched() {
    let f = Fixture::new();
    f.populate();
    for name in [
        "",
        ".",
        "..",
        "../Outside",
        "bad/name",
        "bad\\name",
        "bad:name",
        "bad.",
        "settings_",
    ] {
        assert!(
            copy_profile(&f.source, &f.server, name, true).is_err(),
            "{name}"
        );
    }
    assert!(copy_profile(&f.source, f.source.parent().unwrap(), "Default", true).is_err());
    assert!(copy_profile(&f.source, &f.source, "Nested", true).is_err());
    let same_server = copy_profile(&f.source, f.source.parent().unwrap(), "Other", true).unwrap();
    assert_eq!(
        fs::read(same_server.destination.join("core_public__.yaml")).unwrap(),
        fs::read(f.source.join("core_public__.yaml")).unwrap()
    );
    assert_eq!(fs::read_dir(&f.server).unwrap().count(), 0);
}

#[cfg(unix)]
#[test]
fn linked_source_paths_fail_before_replacing_the_destination() {
    use std::os::unix::fs::symlink;
    let f = Fixture::new();
    f.populate();
    let external = f.root.join("outside.txt");
    fs::write(&external, b"do not follow").unwrap();
    symlink(&external, f.source.join("linked.txt")).unwrap();
    let target = f.server.join("settings_Default");
    fs::create_dir(&target).unwrap();
    fs::write(target.join("keep.txt"), b"keep").unwrap();
    assert!(copy_profile(&f.source, &f.server, "Default", true).is_err());
    assert_eq!(fs::read(target.join("keep.txt")).unwrap(), b"keep");
    assert_eq!(fs::read(&external).unwrap(), b"do not follow");
    assert_eq!(fs::read_dir(&f.server).unwrap().count(), 1);
}
