//! The registry, with keys written as `HKCU\...` or `HKEY_CURRENT_USER\...` (and `HKLM`,
//! `HKEY_LOCAL_MACHINE`), as `reg.exe` and PowerShell wrote them.

use super::ffi::*;
use super::{error_text, sandbox, wide};

/// A value of the registry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Value {
    Str(String),
    /// `REG_EXPAND_SZ`, as stored (`%USERPROFILE%\...` not expanded).
    ExpandStr(String),
    Dword(u32),
    /// Another type (binary, multi-string...), not used here.
    Other(u32),
}

impl Value {
    /// The text of a string value.
    pub fn text(&self) -> Option<&str> {
        match self {
            Value::Str(s) | Value::ExpandStr(s) => Some(s),
            _ => None,
        }
    }
}

/// Root key and path inside it of a key written `HKCU\Software\...`; in the sandbox of the
/// tests, both roots are redirected under `HKCU\Software\NEXUL-Tests\<sandbox>\`.
fn split(key: &str, sandbox_name: Option<&str>) -> Result<(HKEY, String), String> {
    let (root, name, rest) = match key.split_once('\\') {
        Some((r, rest)) => match r.to_ascii_uppercase().as_str() {
            "HKCU" | "HKEY_CURRENT_USER" => (HKEY_CURRENT_USER, "HKCU", rest),
            "HKLM" | "HKEY_LOCAL_MACHINE" => (HKEY_LOCAL_MACHINE, "HKLM", rest),
            _ => return Err(format!("clé du registre non prise en charge : {key}")),
        },
        None => return Err(format!("clé du registre non prise en charge : {key}")),
    };
    Ok(match sandbox_name {
        Some(s) => (HKEY_CURRENT_USER, format!(r"Software\NEXUL-Tests\{s}\{name}\{rest}")),
        None => (root, rest.to_string()),
    })
}

fn resolve(key: &str) -> Result<(HKEY, String), String> {
    let sb =
        sandbox().map(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "sandbox".into()));
    split(key, sb.as_deref())
}

struct Key(HKEY);

impl Drop for Key {
    fn drop(&mut self) {
        unsafe { RegCloseKey(self.0) };
    }
}

fn open(key: &str, access: u32) -> Option<Key> {
    let (root, sub) = resolve(key).ok()?;
    let mut h: HKEY = 0;
    let r = unsafe { RegOpenKeyExW(root, wide(&sub).as_ptr(), 0, access, &mut h) };
    (r == ERROR_SUCCESS).then_some(Key(h))
}

fn create(key: &str) -> Result<Key, String> {
    let (root, sub) = resolve(key)?;
    let mut h: HKEY = 0;
    // A concurrent deletion of an ancestor can temporarily leave it marked for
    // deletion (1018). Retry only that transient error, with a bounded delay.
    let mut r = ERROR_SUCCESS;
    for attempt in 0..20 {
        r = unsafe { RegCreateKeyExW(
            root,
            wide(&sub).as_ptr(),
            0,
            std::ptr::null_mut(),
            0,
            KEY_ALL_ACCESS,
            std::ptr::null(),
            &mut h,
            std::ptr::null_mut(),
        ) };
        if r != 1018 || attempt == 19 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    if r == ERROR_SUCCESS {
        Ok(Key(h))
    } else {
        Err(format!("impossible de créer la clé {key} : {}", error_text(r as u32)))
    }
}

/// Writes a value (`None`: the default value of the key), creating the key if needed.
pub fn set(key: &str, name: Option<&str>, value: &Value) -> Result<(), String> {
    let k = create(key)?;
    let (ty, data): (u32, Vec<u8>) = match value {
        Value::Str(s) => (REG_SZ, wide(s).iter().flat_map(|c| c.to_le_bytes()).collect()),
        Value::ExpandStr(s) => (REG_EXPAND_SZ, wide(s).iter().flat_map(|c| c.to_le_bytes()).collect()),
        Value::Dword(d) => (REG_DWORD, d.to_le_bytes().to_vec()),
        Value::Other(_) => return Err("type de valeur non pris en charge".into()),
    };
    let name_w = name.map(wide);
    let name_p = name_w.as_ref().map_or(std::ptr::null(), |n| n.as_ptr());
    let r = unsafe { RegSetValueExW(k.0, name_p, 0, ty, data.as_ptr(), data.len() as u32) };
    if r == ERROR_SUCCESS {
        Ok(())
    } else {
        Err(format!("impossible d'écrire {}\\{} : {}", key, name.unwrap_or("(par défaut)"), error_text(r as u32)))
    }
}

/// Reads a value (`None` if the key or the value does not exist).
pub fn get(key: &str, name: Option<&str>) -> Option<Value> {
    let k = open(key, KEY_READ)?;
    let name_w = name.map(wide);
    let name_p = name_w.as_ref().map_or(std::ptr::null(), |n| n.as_ptr());
    let mut ty = 0u32;
    let mut len = 0u32;
    // First the size, then the data; the value may grow in between: retry.
    if unsafe { RegQueryValueExW(k.0, name_p, std::ptr::null_mut(), &mut ty, std::ptr::null_mut(), &mut len) }
        != ERROR_SUCCESS
    {
        return None;
    }
    let mut buf = vec![0u8; len as usize];
    loop {
        let mut l = buf.len() as u32;
        match unsafe { RegQueryValueExW(k.0, name_p, std::ptr::null_mut(), &mut ty, buf.as_mut_ptr(), &mut l) } {
            ERROR_SUCCESS => {
                buf.truncate(l as usize);
                break;
            }
            ERROR_MORE_DATA => buf = vec![0; l as usize],
            _ => return None,
        }
    }
    let text = || {
        let units: Vec<u16> = buf.as_chunks::<2>().0.iter().map(|c| u16::from_le_bytes(*c)).collect();
        super::from_wide(&units)
    };
    Some(match ty {
        REG_SZ => Value::Str(text()),
        REG_EXPAND_SZ => Value::ExpandStr(text()),
        REG_DWORD if buf.len() >= 4 => Value::Dword(u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]])),
        other => Value::Other(other),
    })
}

/// Reads a string value (expanded if it is `REG_EXPAND_SZ`).
pub fn get_text(key: &str, name: Option<&str>) -> Option<String> {
    match get(key, name)? {
        Value::Str(s) => Some(s),
        Value::ExpandStr(s) => Some(super::env::expand(&s)),
        _ => None,
    }
}

/// Removes a value; a missing value is not an error.
pub fn delete_value(key: &str, name: &str) -> Result<(), String> {
    let Some(k) = open(key, KEY_ALL_ACCESS) else { return Ok(()) };
    match unsafe { RegDeleteValueW(k.0, wide(name).as_ptr()) } {
        ERROR_SUCCESS | ERROR_FILE_NOT_FOUND => Ok(()),
        r => Err(format!("impossible de supprimer {key}\\{name} : {}", error_text(r as u32))),
    }
}

/// Removes a key and everything under it; a missing key is not an error.
pub fn delete_tree(key: &str) -> Result<(), String> {
    let (root, sub) = resolve(key)?;
    let w = wide(&sub);
    let r = unsafe { RegDeleteTreeW(root, w.as_ptr()) };
    // The tree is empty now; the key itself, if `RegDeleteTreeW` kept it.
    let r2 = unsafe { RegDeleteKeyW(root, w.as_ptr()) };
    match (r, r2) {
        (ERROR_SUCCESS | ERROR_FILE_NOT_FOUND, ERROR_SUCCESS | ERROR_FILE_NOT_FOUND) => Ok(()),
        (e, _) if e != ERROR_SUCCESS && e != ERROR_FILE_NOT_FOUND => {
            Err(format!("impossible de supprimer la clé {key} : {}", error_text(e as u32)))
        }
        (_, e) => Err(format!("impossible de supprimer la clé {key} : {}", error_text(e as u32))),
    }
}

/// The names of the subkeys of a key (empty if it does not exist).
pub fn subkeys(key: &str) -> Vec<String> {
    let Some(k) = open(key, KEY_READ) else { return Vec::new() };
    let mut out = Vec::new();
    let mut buf = [0u16; 512];
    for i in 0.. {
        let mut len = buf.len() as u32;
        let r = unsafe {
            RegEnumKeyExW(
                k.0,
                i,
                buf.as_mut_ptr(),
                &mut len,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        match r {
            ERROR_SUCCESS => out.push(String::from_utf16_lossy(&buf[..len as usize])),
            ERROR_MORE_DATA => continue, // a name longer than 512 characters: not ours
            _ => break,
        }
    }
    out
}

/// `HKCU\...` written in full (`HKEY_CURRENT_USER\...`), as PowerShell names the keys.
pub fn long_name(key: &str) -> String {
    match key.split_once('\\') {
        Some((r, rest)) if r.eq_ignore_ascii_case("HKCU") => format!(r"HKEY_CURRENT_USER\{rest}"),
        Some((r, rest)) if r.eq_ignore_ascii_case("HKLM") => format!(r"HKEY_LOCAL_MACHINE\{rest}"),
        _ => key.to_string(),
    }
}

/// The parent of the keys of the tests, `HKCU\Software\NEXUL-Tests`, removed once no test
/// uses it any more (a key with subkeys is not removed: other tests may be running).
#[cfg(test)]
pub fn remove_tests_root_if_empty() {
    unsafe { RegDeleteKeyW(HKEY_CURRENT_USER, wide(r"Software\NEXUL-Tests").as_ptr()) };
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A key of the tests, removed at the end (never a setting of the user).
    struct TestKey(String);

    impl TestKey {
        fn new(name: &str) -> TestKey {
            static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let sequence = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let timestamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
            TestKey(format!(r"HKCU\Software\NEXUL-Tests\unit-{}-{timestamp}-{sequence}-{name}", std::process::id()))
        }
    }

    impl Drop for TestKey {
        fn drop(&mut self) {
            let _ = delete_tree(&self.0);
            remove_tests_root_if_empty();
        }
    }

    #[test]
    fn roots_and_sandbox() {
        assert_eq!(split(r"HKCU\Software\X", None).unwrap(), (HKEY_CURRENT_USER, r"Software\X".into()));
        assert_eq!(split(r"HKEY_LOCAL_MACHINE\SOFTWARE\Y", None).unwrap(), (HKEY_LOCAL_MACHINE, r"SOFTWARE\Y".into()));
        assert_eq!(
            split(r"hklm\SOFTWARE\Y", Some("sb1")).unwrap(),
            (HKEY_CURRENT_USER, r"Software\NEXUL-Tests\sb1\HKLM\SOFTWARE\Y".into())
        );
        assert!(split(r"HKCR\.nx", None).is_err());
        assert!(split("HKCU", None).is_err());
        assert_eq!(long_name(r"HKCU\Software\A"), r"HKEY_CURRENT_USER\Software\A");
        assert_eq!(long_name(r"HKEY_LOCAL_MACHINE\B"), r"HKEY_LOCAL_MACHINE\B");
    }

    #[test]
    fn values_round_trip() {
        let k = TestKey::new("values");
        assert_eq!(get(&k.0, Some("absent")), None);
        let long = "é".repeat(3000);
        set(&k.0, None, &Value::Str("défaut".into())).unwrap();
        set(&k.0, Some("texte"), &Value::Str("C:\\Programmes\\NEXUL été 😀".into())).unwrap();
        set(&k.0, Some("long"), &Value::Str(long.clone())).unwrap();
        set(&k.0, Some("vide"), &Value::Str(String::new())).unwrap();
        set(&k.0, Some("chemin"), &Value::ExpandStr(r"%USERPROFILE%\nexul".into())).unwrap();
        set(&k.0, Some("taille"), &Value::Dword(123456)).unwrap();
        assert_eq!(get(&k.0, None), Some(Value::Str("défaut".into())));
        assert_eq!(get(&k.0, Some("texte")), Some(Value::Str("C:\\Programmes\\NEXUL été 😀".into())));
        assert_eq!(get(&k.0, Some("long")), Some(Value::Str(long)));
        assert_eq!(get(&k.0, Some("vide")), Some(Value::Str(String::new())));
        assert_eq!(get(&k.0, Some("chemin")), Some(Value::ExpandStr(r"%USERPROFILE%\nexul".into())));
        let expanded = get_text(&k.0, Some("chemin")).unwrap();
        assert!(!expanded.contains('%') && expanded.ends_with(r"\nexul"), "{expanded}");
        assert_eq!(get(&k.0, Some("taille")), Some(Value::Dword(123456)));
        // Replacing a value, removing one.
        set(&k.0, Some("texte"), &Value::Str("autre".into())).unwrap();
        assert_eq!(get_text(&k.0, Some("texte")).as_deref(), Some("autre"));
        delete_value(&k.0, "texte").unwrap();
        assert_eq!(get(&k.0, Some("texte")), None);
        delete_value(&k.0, "texte").unwrap();
    }

    #[test]
    fn keys_tree() {
        let k = TestKey::new("tree");
        for sub in ["a", "b", r"b\c\d", "NEXUL été"] {
            set(&format!(r"{}\{sub}", k.0), Some("v"), &Value::Dword(1)).unwrap();
        }
        let mut names = subkeys(&k.0);
        names.sort();
        assert_eq!(names, ["NEXUL été", "a", "b"]);
        delete_tree(&format!(r"{}\b", k.0)).unwrap();
        assert_eq!(get(&format!(r"{}\b\c\d", k.0), Some("v")), None);
        assert!(!subkeys(&k.0).contains(&"b".to_string()));
        delete_tree(&format!(r"{}\b", k.0)).unwrap(); // missing: no error
        delete_tree(&k.0).unwrap();
        assert!(subkeys(&k.0).is_empty());
        assert_eq!(get(&k.0, Some("v")), None);
        assert!(subkeys(r"HKCU\Software\NEXUL-Tests\nothing-here").is_empty());
    }
}
