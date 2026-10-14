//! Windows resources (icon, version information, manifest) compiled with `llvm-rc`
//! when LLVM is available; without it, the executable simply has no icon.

use std::path::{Path, PathBuf};

fn find_llvm_rc() -> Option<PathBuf> {
    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Some(v) = std::env::var_os("NEXUL_LLVM") {
        dirs.push(PathBuf::from(&v).join("bin"));
        dirs.push(PathBuf::from(v));
    }
    dirs.push(PathBuf::from(r"C:\Program Files\LLVM\bin"));
    if let Some(p) = std::env::var_os("PATH") {
        dirs.extend(std::env::split_paths(&p));
    }
    dirs.into_iter().map(|d| d.join("llvm-rc.exe")).find(|p| p.is_file())
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../studio/assets/nexul-studio.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let Some(rc) = find_llvm_rc() else { return };
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let windres = out.join("llvm-windres.exe");
    if std::fs::copy(&rc, &windres).is_err() {
        return;
    }
    let icon = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("studio").join("assets").join("nexul-studio.ico");
    let manifest = out.join("app.manifest");
    std::fs::write(
        &manifest,
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3"><security><requestedPrivileges><requestedExecutionLevel level="asInvoker" uiAccess="false"/></requestedPrivileges></security></trustInfo>
  <application xmlns="urn:schemas-microsoft-com:asm.v3"><windowsSettings><dpiAware xmlns="http://schemas.microsoft.com/SMI/2005/WindowsSettings">true/pm</dpiAware></windowsSettings></application>
</assembly>
"#,
    )
    .unwrap();
    let v: Vec<u32> = env!("CARGO_PKG_VERSION").split('.').map(|x| x.parse().unwrap_or(0)).collect();
    let ver = format!("{},{},{},0", v[0], v[1], v[2]);
    let ver_s = format!("{}.{}.{}.0", v[0], v[1], v[2]);
    let rc_text = format!(
        "1 ICON \"{icon}\"\n1 24 \"{manifest}\"\n1 VERSIONINFO\nFILEVERSION {ver}\nPRODUCTVERSION {ver}\nFILEOS 0x40004\nFILETYPE 0x1\nBEGIN\n  BLOCK \"StringFileInfo\"\n  BEGIN\n    BLOCK \"040C04B0\"\n    BEGIN\n      VALUE \"CompanyName\", \"NEXUL\"\n      VALUE \"FileDescription\", \"Installation de NEXUL\"\n      VALUE \"FileVersion\", \"{ver_s}\"\n      VALUE \"ProductName\", \"NEXUL\"\n      VALUE \"ProductVersion\", \"{ver_s}\"\n      VALUE \"LegalCopyright\", \"MIT OR Apache-2.0\"\n    END\n  END\n  BLOCK \"VarFileInfo\"\n  BEGIN\n    VALUE \"Translation\", 0x040C, 1200\n  END\nEND\n",
        icon = icon.display().to_string().replace('\\', "\\\\"),
        manifest = manifest.display().to_string().replace('\\', "\\\\"),
    );
    let rc_file = out.join("app.rc");
    std::fs::write(&rc_file, rc_text).unwrap();
    let obj = out.join("app-resources.o");
    let ok = std::process::Command::new(&windres)
        .args(["--no-preprocess", "--output-format=coff", "-i"])
        .arg(&rc_file)
        .arg("-o")
        .arg(&obj)
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if ok {
        println!("cargo:rustc-link-arg-bin=NEXUL-Setup={}", obj.display());
    }
}
