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
fn install_update_uninstall() {
    let sb = Sandbox::new("full");
    let dir = sb.path(r"LocalAppData\Programs\NEXUL");
    let bin = dir.join("bin");
    let dir_s = dir.display().to_string();
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
    assert_eq!(PathBuf::from(shortcut_target(&sb.path(r"Desktop\NEXUL Studio.lnk"))), studio);
    assert_eq!(PathBuf::from(shortcut_target(&sb.path(r"Programs\NEXUL\NEXUL Studio.lnk"))), studio);
    assert_eq!(
        PathBuf::from(shortcut_target(&sb.path(r"Programs\NEXUL\Désinstaller NEXUL.lnk"))),
        dir.join(UNINSTALLER)
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
