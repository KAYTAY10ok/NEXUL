//! The environment variables of the user and of the machine, stored in the registry, as
//! `[Environment]::SetEnvironmentVariable` handled them: the PATH keeps its raw form
//! (`%USERPROFILE%\...` is not expanded) and stays `REG_EXPAND_SZ`; the programs started
//! afterwards are told of the change (`WM_SETTINGCHANGE`).

use super::ffi::*;
use super::reg::{self, Value};
use super::{from_wide, sandbox, wide};

pub const USER_KEY: &str = r"HKCU\Environment";
pub const MACHINE_KEY: &str = r"HKLM\SYSTEM\CurrentControlSet\Control\Session Manager\Environment";

/// The key of the variables of `scope`: `User` (default) or `Machine`.
pub fn key_of(scope: &str) -> &'static str {
    if scope == "Machine" {
        MACHINE_KEY
    } else {
        USER_KEY
    }
}

/// Expands the `%VARIABLES%` of `s` with the environment of this process.
pub fn expand(s: &str) -> String {
    let src = wide(s);
    let mut buf = vec![0u16; 1024];
    loop {
        let n = unsafe { ExpandEnvironmentStringsW(src.as_ptr(), buf.as_mut_ptr(), buf.len() as u32) } as usize;
        if n == 0 {
            return s.to_string();
        }
        if n <= buf.len() {
            return from_wide(&buf);
        }
        buf = vec![0; n];
    }
}

/// A folder for comparisons: no quotes, `\`, no final `\`, lowercase.
fn norm(p: &str) -> String {
    p.trim().trim_matches('"').replace('/', "\\").trim_end_matches('\\').to_lowercase()
}

/// The new raw value of a PATH: the entries that are, once expanded, one of the folders
/// `remove` or inside one of them are removed, as are the empty entries; `add` is added at
/// the end. The other entries are kept exactly as written.
pub fn edit_path(raw: &str, remove: &[String], add: Option<&str>) -> String {
    let rm: Vec<String> = remove.iter().map(|r| norm(r)).filter(|r| !r.is_empty()).collect();
    let mut parts: Vec<&str> = raw
        .split(';')
        .filter(|p| !p.trim().is_empty())
        .filter(|p| {
            let e = norm(&expand(p));
            !rm.iter().any(|r| e == *r || e.starts_with(&format!("{r}\\")))
        })
        .collect();
    if let Some(a) = add {
        parts.push(a);
    }
    parts.join(";")
}

/// [`edit_path`] on the `Path` value of `key`; `true` if it changed.
pub fn change_path_in(key: &str, remove: &[String], add: Option<&str>) -> Result<bool, String> {
    let raw = reg::get(key, Some("Path")).and_then(|v| v.text().map(str::to_string)).unwrap_or_default();
    let new = edit_path(&raw, remove, add);
    if new == raw {
        return Ok(false);
    }
    reg::set(key, Some("Path"), &Value::ExpandStr(new))?;
    Ok(true)
}

/// Removes from the PATH of `scope` the folders `remove` (and those inside them), then adds
/// `add`. `Machine` needs the rights of an administrator.
pub fn change_path(scope: &str, remove: &[String], add: Option<&str>) -> Result<(), String> {
    if change_path_in(key_of(scope), remove, add).map_err(|e| format!("PATH : {e}"))? {
        broadcast();
    }
    Ok(())
}

/// The entries of the PATH of `scope`, expanded.
pub fn path_entries(scope: &str) -> Vec<String> {
    let raw = reg::get(key_of(scope), Some("Path")).and_then(|v| v.text().map(str::to_string)).unwrap_or_default();
    raw.split(';').map(|p| expand(p).trim().to_string()).filter(|p| !p.is_empty()).collect()
}

/// A variable of the user, expanded.
pub fn user_var(name: &str) -> Option<String> {
    reg::get_text(USER_KEY, Some(name)).filter(|v| !v.is_empty())
}

pub fn set_user_var(name: &str, value: &str) -> Result<(), String> {
    reg::set(USER_KEY, Some(name), &Value::Str(value.to_string()))?;
    broadcast();
    Ok(())
}

pub fn remove_user_var(name: &str) -> Result<(), String> {
    reg::delete_value(USER_KEY, name)?;
    broadcast();
    Ok(())
}

/// Tells the programs (Explorer first) that the environment changed, so that the programs
/// they start see it. Each window has 1 s to answer, as with
/// `[Environment]::SetEnvironmentVariable` before. Not in the sandbox of the tests.
pub fn broadcast() {
    if sandbox().is_some() {
        return;
    }
    let what = wide("Environment");
    let mut result = 0usize;
    unsafe {
        SendMessageTimeoutW(
            HWND_BROADCAST,
            WM_SETTINGCHANGE,
            0,
            what.as_ptr() as isize,
            SMTO_ABORTIFHUNG,
            1000,
            &mut result,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expanding() {
        let profile = std::env::var("USERPROFILE").unwrap();
        assert_eq!(expand(r"%USERPROFILE%\x"), format!(r"{profile}\x"));
        assert_eq!(expand("sans variable"), "sans variable");
        assert_eq!(expand("%NEXUL_VARIABLE_INEXISTANTE%"), "%NEXUL_VARIABLE_INEXISTANTE%");
        let long = format!("%USERPROFILE%{}", "a".repeat(5000));
        assert_eq!(expand(&long).len(), profile.len() + 5000);
    }

    #[test]
    fn editing_the_path() {
        let profile = std::env::var("USERPROFILE").unwrap();
        let raw = format!(
            r"C:\Windows;%USERPROFILE%\.cargo\bin;C:\NEXUL\bin;;c:/nexul/bin/;C:\NEXUL\bin\sub;C:\NEXUL Studio;{profile}\AppData\Local\Programs\NEXUL\bin\"
        );
        // Removal: the folder, written in any case or form, and the folders inside it;
        // the other entries keep their raw form; empty entries go.
        assert_eq!(
            edit_path(&raw, &[r"C:\NEXUL\bin".into()], None),
            format!(r"C:\Windows;%USERPROFILE%\.cargo\bin;C:\NEXUL Studio;{profile}\AppData\Local\Programs\NEXUL\bin\")
        );
        // An entry written with a variable is compared once expanded.
        assert_eq!(
            edit_path(&raw, &[format!(r"{profile}\.cargo")], None),
            format!(
                r"C:\Windows;C:\NEXUL\bin;c:/nexul/bin/;C:\NEXUL\bin\sub;C:\NEXUL Studio;{profile}\AppData\Local\Programs\NEXUL\bin\"
            )
        );
        // Install: the folder is moved to the end, once.
        let bin = format!(r"{profile}\AppData\Local\Programs\NEXUL\bin");
        let once = edit_path(&raw, std::slice::from_ref(&bin), Some(&bin));
        assert!(once.ends_with(&format!(";{bin}")));
        assert_eq!(once.matches(r"Programs\NEXUL\bin").count(), 1);
        assert_eq!(edit_path(&once, std::slice::from_ref(&bin), Some(&bin)), once);
        // Nothing to remove, an empty PATH, an empty folder to remove.
        assert_eq!(edit_path("", &[], Some(r"C:\x")), r"C:\x");
        assert_eq!(edit_path(r"C:\a;C:\b", &[String::new()], None), r"C:\a;C:\b");
        assert_eq!(edit_path(r"C:\a;C:\ab", &[r"C:\a".into()], None), r"C:\ab");
        assert_eq!(edit_path(r#""C:\a b";C:\c"#, &[r"C:\a b".into()], None), r"C:\c");
    }

    #[test]
    fn path_in_the_registry() {
        let _root = reg::TestsRootUse::new();
        let key = format!(r"HKCU\Software\NEXUL-Tests\unit-{}-env", std::process::id());
        reg::set(&key, Some("Path"), &Value::Str(r"%SystemRoot%;C:\old\bin".into())).unwrap();
        assert_eq!(change_path_in(&key, &[r"C:\old".into()], Some(r"%USERPROFILE%\nexul\bin")), Ok(true));
        assert_eq!(
            reg::get(&key, Some("Path")),
            Some(Value::ExpandStr(r"%SystemRoot%;%USERPROFILE%\nexul\bin".into()))
        );
        // Nothing changes: nothing written.
        assert_eq!(change_path_in(&key, &[r"C:\absent".into()], None), Ok(false));
        assert_eq!(
            reg::get(&key, Some("Path")),
            Some(Value::ExpandStr(r"%SystemRoot%;%USERPROFILE%\nexul\bin".into()))
        );
        reg::delete_tree(&key).unwrap();
        // A missing value: created.
        assert_eq!(change_path_in(&key, &[], Some(r"C:\n\bin")), Ok(true));
        assert_eq!(reg::get_text(&key, Some("Path")).as_deref(), Some(r"C:\n\bin"));
        reg::delete_tree(&key).unwrap();
        assert_eq!(key_of("Machine"), MACHINE_KEY);
        assert_eq!(key_of("User"), USER_KEY);
    }
}
