//! The real installer, from start to end, without window: installation, update over an
//! earlier installation and an old copy of NEXUL Studio, uninstallation.
//!
//! Everything happens in a sandbox (`NEXUL_SETUP_SANDBOX`, feature `test-sandbox`): the
//! registry is redirected under `HKCU\Software\NEXUL-Tests\<sandbox>\`, the Desktop and the
//! Start menu into the sandbox folder, and `ProgramFiles`, `LOCALAPPDATA`, `APPDATA`,
//! `USERPROFILE` and `TEMP` too. The results are checked independently of the installer:
//! with `reg.exe` and, for the shortcuts, with PowerShell (here only, in the test).

mod common;

use common::*;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

#[test]
fn modified_elevation_job_cannot_delete_uninventoried_files() {
    let sb = Sandbox::new("elevation-job");
    let old = sb.path(r"ProgramFiles\NEXUL Studio");
    std::fs::create_dir_all(&old).unwrap();
    std::fs::write(old.join("NEXUL Studio.exe"), b"MZ").unwrap();
    let victim_dir = sb.path("Elsewhere");
    let victim = victim_dir.join("important.txt");
    std::fs::write(&victim, "keep this file").unwrap();
    let job = sb.path(r"Temp\job.txt");
    let log = sb.path(r"Temp\job.log");
    std::fs::write(&log, "do not overwrite caller files with elevated rights").unwrap();
    let selected = format!("choice\tfolder\t\tremove\t{}", old.display());
    for forged in [
        format!("del\t{}", victim.display()),
        format!("rmdir\t{}", victim_dir.display()),
        format!("choice\tfolder\t\tremove\t{}", victim_dir.display()),
        format!("{selected}\ndel\t{}", victim.display()),
    ] {
        std::fs::write(&job, forged).unwrap();
        let status = sb.command(Path::new(SETUP)).arg("--old-versions-job").arg(&job).status().unwrap();
        assert_eq!(status.code(), Some(2));
        assert_eq!(std::fs::read_to_string(&victim).unwrap(), "keep this file");
        assert!(old.is_dir(), "an invalid job must be rejected before any operation");
    }
    // A real fresh inventory still allows the selected NEXUL copy to be removed.
    std::fs::write(&job, selected).unwrap();
    let status = sb.command(Path::new(SETUP)).arg("--old-versions-job").arg(&job).status().unwrap();
    assert!(status.success());
    assert!(!old.exists());
    assert_eq!(std::fs::read_to_string(&victim).unwrap(), "keep this file");
    assert_eq!(std::fs::read_to_string(&log).unwrap(), "do not overwrite caller files with elevated rights");
}

#[test]
fn elevated_job_rejects_manifest_paths_outside_installation() {
    let sb = Sandbox::new("elevation-manifest");
    // This fake user installation is found by the fresh scan, without any forged
    // operation or registry entry. Its manifest must still be treated as untrusted.
    let dir = sb.path(r"LocalAppData\Programs\NEXUL");
    std::fs::create_dir_all(dir.join("bin")).unwrap();
    let installed = dir.join("installed.txt");
    let user_file = dir.join("notes.txt");
    let absolute_victim = sb.path(r"Elsewhere\important.txt");
    let parent_victim = dir.parent().unwrap().join("parent.txt");
    std::fs::write(&installed, "installed file").unwrap();
    std::fs::write(&user_file, "user file").unwrap();
    std::fs::write(&absolute_victim, "absolute sentinel").unwrap();
    std::fs::write(&parent_victim, "parent sentinel").unwrap();
    let job = sb.path(r"Temp\job.txt");
    std::fs::write(&job, format!("choice\tinstall\t\tremove\t{}", dir.display())).unwrap();
    for unsafe_path in [
        absolute_victim.display().to_string(),
        "../parent.txt".into(),
        r"..\parent.txt".into(),
        r"bin\..\..\parent.txt".into(),
    ] {
        std::fs::write(dir.join("install.txt"), format!("version\t0.1\nfile\tinstalled.txt\nfile\t{unsafe_path}\n")).unwrap();
        let status = sb.command(Path::new(SETUP)).arg("--old-versions-job").arg(&job).status().unwrap();
        assert_eq!(status.code(), Some(2), "manifest entry: {unsafe_path}");
        assert_eq!(std::fs::read_to_string(&absolute_victim).unwrap(), "absolute sentinel");
        assert_eq!(std::fs::read_to_string(&parent_victim).unwrap(), "parent sentinel");
        assert_eq!(std::fs::read_to_string(&installed).unwrap(), "installed file", "validate all entries before removing any file");
        assert_eq!(std::fs::read_to_string(&user_file).unwrap(), "user file");
        assert!(dir.join("install.txt").is_file());
    }
    // A safe manifest still removes its own files and keeps files added by the user.
    std::fs::write(dir.join("install.txt"), "version\t0.1\nfile\tinstalled.txt\n").unwrap();
    let status = sb.command(Path::new(SETUP)).arg("--old-versions-job").arg(&job).status().unwrap();
    assert!(status.success());
    assert!(!installed.exists());
    assert!(!dir.join("install.txt").exists());
    assert_eq!(std::fs::read_to_string(&user_file).unwrap(), "user file");
    assert_eq!(std::fs::read_to_string(&absolute_victim).unwrap(), "absolute sentinel");
    assert_eq!(std::fs::read_to_string(&parent_victim).unwrap(), "parent sentinel");
}

#[test]
fn elevated_job_rejects_manifest_junction_outside_installation() {
    let sb = Sandbox::new("elevation-junction");
    let dir = sb.path(r"LocalAppData\Programs\NEXUL");
    std::fs::create_dir_all(dir.join("bin")).unwrap();
    let installed = dir.join("installed.txt");
    let outside = sb.path("Elsewhere");
    let victim = outside.join("important.txt");
    let junction = dir.join("linked");
    std::fs::write(&installed, "installed file").unwrap();
    std::fs::write(&victim, "junction sentinel").unwrap();
    // Directory junctions do not require the symbolic-link administrator privilege.
    let created = Command::new("cmd").args(["/C", "mklink", "/J"]).arg(&junction).arg(&outside).output().unwrap();
    assert!(created.status.success(), "junction creation failed: {}", String::from_utf8_lossy(&created.stderr));
    assert_eq!(std::fs::read_to_string(junction.join("important.txt")).unwrap(), "junction sentinel");
    std::fs::write(dir.join("install.txt"), "version\t0.1\nfile\tinstalled.txt\nfile\tlinked/important.txt\n").unwrap();
    let job = sb.path(r"Temp\job.txt");
    std::fs::write(&job, format!("choice\tinstall\t\tremove\t{}", dir.display())).unwrap();
    let status = sb.command(Path::new(SETUP)).arg("--old-versions-job").arg(&job).status().unwrap();
    assert_eq!(status.code(), Some(2));
    assert_eq!(std::fs::read_to_string(&victim).unwrap(), "junction sentinel");
    assert_eq!(std::fs::read_to_string(&installed).unwrap(), "installed file");
    assert!(dir.join("install.txt").is_file());
    // Remove only the junction, with its exact path inside this test's sandbox.
    std::fs::remove_dir(&junction).unwrap();
    assert_eq!(std::fs::read_to_string(&victim).unwrap(), "junction sentinel");
}

#[test]
fn install_update_uninstall() {
    let sb = Sandbox::new("full");
    let dir = sb.path(r"LocalAppData\Programs\NEXUL");
    let bin = dir.join("bin");
    let dir_s = dir.display().to_string();
    let settings_dir = sb.path(r"AppData\NEXUL Studio");
    std::fs::create_dir_all(&settings_dir).unwrap();
    std::fs::write(
        settings_dir.join("state.json"),
        r#"{"settings":{"theme":"light","lastRoot":"C:\\Projets\\Été","sidebarOpen":false,"panelOpen":true,"autoSave":false},"roots":{"C:\\Projets\\Été":{"session":{"open":["src\\main.nx"],"active":"src\\main.nx"}}}}"#,
    )
    .unwrap();
    let native_settings = settings_dir.join("native.json");
    let uninstall_key = sb.key(r"HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\NEXUL");
    let env_key = sb.key(r"HKCU\Environment");
    let path_value = || reg_value(&env_key, Some("Path")).map(|(_, v)| v).unwrap_or_default();

    // ---- Installation.
    let v1 = pack(
        &sb,
        "setup-1",
        &[
            ("core", "bin/nxc.exe", None),
            ("core", "README.md", Some("version 1")),
            ("studio", "bin/NEXUL Studio.exe", None),
            ("samples", "examples/bonjour.nx", Some("fn main() {}\n")),
            ("samples", "examples/ancien.nx", Some("// retiré en version 2\n")),
        ],
    );
    sb.run_silent(&v1, &["--dir", &dir_s]);

    assert_eq!(std::fs::read_to_string(dir.join("README.md")).unwrap(), "version 1");
    assert!(dir.join(r"examples\ancien.nx").is_file());
    let migrated = std::fs::read_to_string(&native_settings).unwrap();
    for value in ["\"theme\":\"light\"", "\"sidebar\":false", "\"panel\":true", "\"autosave\":\"off\"", "\"last_root\":\"C:\\\\Projets\\\\Été\""] {
        assert!(migrated.contains(value), "native settings missing {value}: {migrated}");
    }
    assert!(migrated.contains("\"active\":\"src\\\\main.nx\""), "{migrated}");
    // A later install must not replace preferences already written by the native Studio.
    let native_preferences = r#"{"last_root":"D:\\Projects","theme":"system","sidebar":true,"panel":false,"minimal":false,"autosave":"delay","sessions":{}}"#;
    std::fs::write(&native_settings, native_preferences).unwrap();
    let record = std::fs::read_to_string(dir.join("install.txt")).unwrap();
    assert!(record.starts_with("version\t"));
    for line in [
        "file\tbin/nxc.exe",
        "file\tbin/NEXUL Studio.exe",
        "option\tpath",
        "option\tdesktop",
        "option\tstartmenu",
        "option\tassociate",
    ] {
        assert!(record.lines().any(|l| l == line), "install.txt: {line}\n{record}");
    }
    // The uninstaller is the installer without its data.
    assert_eq!(std::fs::metadata(dir.join(UNINSTALLER)).unwrap().len(), std::fs::metadata(SETUP).unwrap().len());

    // Registration, PATH, NEXUL_HOME, association: in the sandbox.
    assert_eq!(reg_value(&uninstall_key, Some("DisplayName")), Some(("REG_SZ".into(), "NEXUL".into())));
    assert_eq!(reg_value(&uninstall_key, Some("InstallLocation")).unwrap().1, dir_s);
    assert_eq!(reg_value(&uninstall_key, Some("EstimatedSize")).unwrap().0, "REG_DWORD");
    assert_eq!(reg_value(&uninstall_key, Some("NoModify")), Some(("REG_DWORD".into(), "0x1".into())));
    assert_eq!(reg_value(&env_key, Some("Path")).unwrap().0, "REG_EXPAND_SZ");
    assert_eq!(path_value(), bin.display().to_string());
    assert_eq!(reg_value(&env_key, Some("NEXUL_HOME")).unwrap().1, dir_s);
    assert_eq!(reg_value(&sb.key(r"HKCU\Software\Classes\.nx"), None).unwrap().1, "NEXUL.Source");
    assert_eq!(
        reg_value(&sb.key(r"HKCU\Software\Classes\NEXUL.Source\shell\open\command"), None).unwrap().1,
        format!("\"{}\" \"%1\"", bin.join("NEXUL Studio.exe").display())
    );
    // Shortcuts: in the sandbox, and they open the right programs.
    let studio = bin.join("NEXUL Studio.exe");
    assert_eq!(
        PathBuf::from(shortcut_target(&sb.path(r"Desktop\NEXUL Studio.lnk"))).canonicalize().unwrap(),
        studio.canonicalize().unwrap()
    );
    assert_eq!(
        PathBuf::from(shortcut_target(&sb.path(r"Programs\NEXUL\NEXUL Studio.lnk"))).canonicalize().unwrap(),
        studio.canonicalize().unwrap()
    );
    assert_eq!(
        PathBuf::from(shortcut_target(&sb.path(r"Programs\NEXUL\Désinstaller NEXUL.lnk"))).canonicalize().unwrap(),
        dir.join(UNINSTALLER).canonicalize().unwrap()
    );

    // Rule 8: the real settings of the user are not touched.
    let real = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\NEXUL";
    assert_ne!(reg_value(real, Some("InstallLocation")).map(|v| v.1), Some(dir_s.clone()));
    assert!(!reg_value(r"HKCU\Environment", Some("Path")).map(|v| v.1).unwrap_or_default().contains(&sb.name));

    // ---- Update, with an old copy of NEXUL Studio (with a shortcut and a PATH entry) and
    // programs running.
    let old = sb.path(r"ProgramFiles\NEXUL Studio");
    std::fs::create_dir_all(&old).unwrap();
    std::fs::copy(SLEEPER, old.join("NEXUL Studio.exe")).unwrap();
    let old_link = sb.path(r"Desktop\NEXUL Studio (ancien).lnk");
    create_shortcut(&old_link, &old.join("NEXUL Studio.exe"));
    let path_with_old = format!("{};{}", path_value(), old.display());
    let ok = Command::new("reg")
        .args(["add", &env_key, "/v", "Path", "/t", "REG_EXPAND_SZ", "/d", &path_with_old, "/f"])
        .output()
        .unwrap();
    assert!(ok.status.success());
    let decoy_dir = sb.path("Elsewhere");
    std::fs::copy(SLEEPER, decoy_dir.join("NEXUL Studio.exe")).unwrap();
    let mut running = sleeper(&sb, &studio);
    let mut decoy = sleeper(&sb, &decoy_dir.join("NEXUL Studio.exe"));
    std::thread::sleep(Duration::from_millis(300));

    let v2 = pack(
        &sb,
        "setup-2",
        &[
            ("core", "bin/nxc.exe", None),
            ("core", "README.md", Some("version 2")),
            ("studio", "bin/NEXUL Studio.exe", None),
            ("samples", "examples/bonjour.nx", Some("fn main() {}\n")),
        ],
    );
    sb.run_silent(&v2, &["--dir", &dir_s]);

    // The Studio of the installation was stopped, not the one elsewhere.
    wait_until("Studio stopped", || running.try_wait().unwrap().is_some());
    assert!(decoy.try_wait().unwrap().is_none(), "the other NEXUL Studio was stopped");
    let _ = decoy.kill();
    let _ = decoy.wait();
    // The installation is replaced: the file of version 1 only is gone.
    assert_eq!(std::fs::read_to_string(dir.join("README.md")).unwrap(), "version 2");
    assert_eq!(std::fs::read_to_string(&native_settings).unwrap(), native_preferences);
    assert!(!dir.join(r"examples\ancien.nx").exists());
    assert!(dir.join(r"examples\bonjour.nx").is_file());
    // The old copy is backed up and detached: no PATH entry, no shortcut.
    assert!(!old.exists());
    let backups: Vec<PathBuf> =
        std::fs::read_dir(sb.path(r"Home\NEXUL - anciennes versions")).unwrap().flatten().map(|e| e.path()).collect();
    assert_eq!(backups.len(), 1, "{backups:?}");
    assert!(backups[0].join("NEXUL Studio.exe").is_file());
    assert!(backups[0].join("LISEZ-MOI - sauvegarde.txt").is_file());
    assert!(!old_link.exists());
    assert_eq!(path_value(), bin.display().to_string(), "PATH after the update");
    assert_eq!(PathBuf::from(shortcut_target(&sb.path(r"Desktop\NEXUL Studio.lnk"))), studio);

    // ---- Uninstallation, with a program running and a file of the user in the folder.
    std::fs::write(dir.join("notes de l'utilisateur.txt"), "à garder").unwrap();
    let mut nxc = sleeper(&sb, &bin.join("nxc.exe"));
    std::thread::sleep(Duration::from_millis(300));
    sb.run_silent(&dir.join(UNINSTALLER), &[]);
    wait_until("nxc stopped", || nxc.try_wait().unwrap().is_some());
    // The copy in %TEMP% removes the uninstaller once it has exited.
    wait_until("uninstaller removed", || !dir.join(UNINSTALLER).exists());
    let left: Vec<String> =
        std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    assert_eq!(left, ["notes de l'utilisateur.txt"]);
    assert!(!key_exists(&uninstall_key));
    assert!(!key_exists(&sb.key(r"HKCU\Software\Classes\.nx")));
    assert!(!key_exists(&sb.key(r"HKCU\Software\Classes\NEXUL.Source")));
    assert_eq!(path_value(), "");
    assert_eq!(reg_value(&env_key, Some("NEXUL_HOME")), None);
    assert_eq!(std::fs::read_to_string(&native_settings).unwrap(), native_preferences);
    assert!(!sb.path(r"Desktop\NEXUL Studio.lnk").exists());
    assert!(!sb.path(r"Programs\NEXUL").exists());

    // The copy left in %TEMP% is removed by the next run of the program.
    let copies = || {
        std::fs::read_dir(sb.path("Temp"))
            .unwrap()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().starts_with("nexul-uninstall-"))
            .count()
    };
    assert_eq!(copies(), 1);
    // (The copy may still be ending: the program removes it as soon as it can.)
    let nothing = sb.path("rien").display().to_string();
    wait_until("copy removed", || {
        sb.run_silent(Path::new(SETUP), &["--dir", &nothing]);
        copies() == 0
    });
}
