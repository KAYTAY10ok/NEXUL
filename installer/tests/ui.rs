//! The native window of the installer, driven by its self-test (`--ui-selftest`, feature
//! `test-sandbox`) as a user would: welcome and licenses, earlier versions backed up,
//! components (the examples unchecked), folder typed, installation, last page; then the
//! uninstaller, with its own window. Everything in a sandbox (see `common`). A screenshot
//! of each page is saved and checked; the window is placed out of the screen.

mod common;

use common::*;
use std::path::Path;
use std::time::Duration;

/// Width, height and number of distinct colors of a 32-bit `.bmp` saved by the self-test.
fn bmp_info(path: &Path) -> (i32, i32, usize) {
    let data = std::fs::read(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    assert_eq!(&data[..2], b"BM", "{}", path.display());
    let w = i32::from_le_bytes(data[18..22].try_into().unwrap());
    let h = i32::from_le_bytes(data[22..26].try_into().unwrap()).abs();
    let pixels = &data[54..];
    assert_eq!(pixels.len(), (w * h * 4) as usize, "{}", path.display());
    let mut colors: Vec<u32> = pixels.as_chunks::<4>().0.iter().step_by(7).map(|c| u32::from_le_bytes([c[0], c[1], c[2], 0])).collect();
    colors.sort_unstable();
    colors.dedup();
    (w, h, colors.len())
}

/// Runs the self-test of `program` and checks that it went to the end. `minimize`: the
/// window is reduced after each capture: the next ones must still succeed.
fn selftest(sb: &Sandbox, program: &Path, shots: &Path, target: &Path, scale: Option<&str>, minimize: bool) {
    std::fs::create_dir_all(shots).unwrap();
    let mut c = sb.command(program);
    c.arg("--ui-selftest").arg(shots).env("NEXUL_SETUP_SELFTEST_DIR", target);
    if let Some(s) = scale {
        c.env("NEXUL_SETUP_UI_SCALE", s);
    }
    if minimize {
        c.env("NEXUL_SETUP_SELFTEST_MINIMIZE", "1");
    }
    let out = c.output().unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success() && stdout.contains("UI-SELFTEST OK"), "{}: {}\n{stdout}\n{stderr}", program.display(), out.status);
}

/// Copies the screenshots to `NEXUL_SETUP_KEEP_SHOTS/<tag>`, if set, to look at them.
fn keep(shots: &Path, tag: &str) {
    if let Some(dest) = std::env::var_os("NEXUL_SETUP_KEEP_SHOTS") {
        let to = Path::new(&dest).join(tag);
        std::fs::create_dir_all(&to).unwrap();
        for e in std::fs::read_dir(shots).unwrap().flatten() {
            std::fs::copy(e.path(), to.join(e.file_name())).unwrap();
        }
    }
}

fn check_shots(shots: &Path, names: &[&str], scale: f32) {
    for name in names {
        let (w, h, colors) = bmp_info(&shots.join(format!("{name}.bmp")));
        // The client area is 860 × 600 at 96 dpi.
        assert_eq!((w, h), ((860.0 * scale).round() as i32, (600.0 * scale).round() as i32), "{name}");
        assert!(colors > 20, "{name}: {colors} colors (empty capture?)");
    }
}

#[test]
fn install_then_uninstall_with_the_window() {
    let sb = Sandbox::new("ui");
    let dir = sb.path(r"LocalAppData\Programs\NEXUL");
    // An old copy of NEXUL Studio, for the page of the earlier versions.
    let old = sb.path(r"ProgramFiles\NEXUL Studio");
    std::fs::create_dir_all(&old).unwrap();
    std::fs::copy(SLEEPER, old.join("NEXUL Studio.exe")).unwrap();
    let setup = pack(
        &sb,
        "setup",
        &[
            ("core", "bin/nxc.exe", None),
            ("core", "README.md", Some("NEXUL")),
            ("studio", "bin/NEXUL Studio.exe", None),
            ("samples", "examples/bonjour.nx", Some("fn main() {}\n")),
        ],
    );
    let shots = sb.path("captures");
    selftest(&sb, &setup, &shots, &dir, Some("1"), false);
    check_shots(&shots, &["1-bienvenue", "2-anciennes-versions", "3-composants", "4-emplacement", "6-termine"], 1.0);
    keep(&shots, "100");

    // What the user chose in the window was done.
    assert!(dir.join(r"bin\nxc.exe").is_file());
    assert!(dir.join(r"bin\NEXUL Studio.exe").is_file());
    assert!(!dir.join("examples").exists(), "the examples were unchecked");
    assert!(!old.exists(), "the old copy was backed up");
    assert_eq!(std::fs::read_dir(sb.path(r"Home\NEXUL - anciennes versions")).unwrap().count(), 1);
    let uninstall_key = sb.key(r"HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\NEXUL");
    assert_eq!(reg_value(&uninstall_key, Some("InstallLocation")).unwrap().1, dir.display().to_string());
    assert_eq!(
        long_path(Path::new(&shortcut_target(&sb.path(r"Desktop\NEXUL Studio.lnk")))),
        long_path(&dir.join(r"bin\NEXUL Studio.exe"))
    );

    // The uninstaller, with its window.
    let shots = sb.path("captures-desinstallation");
    selftest(&sb, &dir.join(UNINSTALLER), &shots, &dir, Some("1"), false);
    check_shots(&shots, &["1-desinstallation", "3-termine"], 1.0);
    keep(&shots, "100-desinstallation");
    wait_until("folder removed", || !dir.exists());
    assert!(!key_exists(&uninstall_key));
    std::thread::sleep(Duration::from_millis(200));
}

/// The same window at 125 % and 150 %: the layout follows the scale. (At 200 % the window
/// is taller than a 1080-pixel screen, which Windows does not allow: a screen set to 200 %
/// has more pixels.)
#[test]
fn the_window_at_other_scales() {
    for (tag, scale) in [("125", 1.25f32), ("150", 1.5)] {
        let sb = Sandbox::new(&format!("ui-{tag}"));
        let dir = sb.path(r"LocalAppData\Programs\NEXUL");
        let setup = pack(&sb, "setup", &[("core", "bin/nxc.exe", None), ("samples", "examples/a.nx", Some("x"))]);
        let shots = sb.path("captures");
        // At 125 %, the window is also reduced after each capture.
        selftest(&sb, &setup, &shots, &dir, Some(&scale.to_string()), tag == "125");
        check_shots(&shots, &["1-bienvenue", "3-composants", "4-emplacement", "6-termine"], scale);
        keep(&shots, tag);
    }
}
