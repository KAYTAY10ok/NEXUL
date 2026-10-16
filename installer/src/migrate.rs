//! One-time migration of settings from the former web Studio to the native Studio.
//!
//! The installer intentionally has no third-party dependencies, so this file contains a
//! small JSON reader for the settings shape it needs to migrate.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, PartialEq)]
enum Json {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<Json>),
    Object(Vec<(String, Json)>),
}

impl Json {
    fn get(&self, name: &str) -> Option<&Json> {
        let Self::Object(fields) = self else {
            return None;
        };
        fields
            .iter()
            .rev()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value)
    }

    fn string(&self) -> Option<&str> {
        if let Self::String(value) = self {
            Some(value)
        } else {
            None
        }
    }

    fn boolean(&self) -> Option<bool> {
        if let Self::Bool(value) = self {
            Some(*value)
        } else {
            None
        }
    }

    fn number(&self) -> Option<f64> {
        if let Self::Number(value) = self {
            Some(*value)
        } else {
            None
        }
    }

    fn strings(&self) -> impl Iterator<Item = &str> {
        let values = match self {
            Self::Array(values) => values.as_slice(),
            _ => &[],
        };
        values.iter().filter_map(Json::string)
    }
}

struct Parser<'a> {
    /// The text and its bytes; `offset` is always on a character boundary.
    text: &'a str,
    input: &'a [u8],
    offset: usize,
    depth: usize,
}

impl Parser<'_> {
    fn whitespace(&mut self) {
        while self
            .input
            .get(self.offset)
            .is_some_and(u8::is_ascii_whitespace)
        {
            self.offset += 1;
        }
    }

    fn byte(&mut self, expected: u8) -> Result<(), String> {
        self.whitespace();
        if self.input.get(self.offset) == Some(&expected) {
            self.offset += 1;
            Ok(())
        } else {
            Err(format!("JSON invalide à l'octet {}", self.offset))
        }
    }

    fn value(&mut self) -> Result<Json, String> {
        self.whitespace();
        self.depth += 1;
        if self.depth > 128 {
            return Err("JSON trop imbriqué".into());
        }
        let result = match self.input.get(self.offset).copied() {
            Some(b'n') => {
                self.literal(b"null")?;
                Ok(Json::Null)
            }
            Some(b't') => {
                self.literal(b"true")?;
                Ok(Json::Bool(true))
            }
            Some(b'f') => {
                self.literal(b"false")?;
                Ok(Json::Bool(false))
            }
            Some(b'"') => self.string().map(Json::String),
            Some(b'[') => self.array(),
            Some(b'{') => self.object(),
            Some(b'-' | b'0'..=b'9') => self.number(),
            _ => Err(format!("JSON invalide à l'octet {}", self.offset)),
        };
        self.depth -= 1;
        result
    }

    fn literal(&mut self, text: &[u8]) -> Result<(), String> {
        if self.input.get(self.offset..self.offset + text.len()) == Some(text) {
            self.offset += text.len();
            Ok(())
        } else {
            Err(format!("JSON invalide à l'octet {}", self.offset))
        }
    }

    fn string(&mut self) -> Result<String, String> {
        self.byte(b'"')?;
        let mut result = String::new();
        loop {
            let Some(&byte) = self.input.get(self.offset) else {
                return Err("chaîne JSON incomplète".into());
            };
            match byte {
                b'"' => {
                    self.offset += 1;
                    return Ok(result);
                }
                b'\\' => {
                    self.offset += 1;
                    let Some(escape) = self.input.get(self.offset).copied() else {
                        return Err("échappement JSON incomplet".into());
                    };
                    self.offset += 1;
                    match escape {
                        b'"' => result.push('"'),
                        b'\\' => result.push('\\'),
                        b'/' => result.push('/'),
                        b'b' => result.push('\u{0008}'),
                        b'f' => result.push('\u{000c}'),
                        b'n' => result.push('\n'),
                        b'r' => result.push('\r'),
                        b't' => result.push('\t'),
                        b'u' => {
                            let first = self.hex_quad()?;
                            let scalar = if (0xd800..=0xdbff).contains(&first) {
                                if self.input.get(self.offset..self.offset + 2) != Some(b"\\u") {
                                    return Err(
                                        "paire de substitution Unicode JSON incomplète".into()
                                    );
                                }
                                self.offset += 2;
                                let second = self.hex_quad()?;
                                if !(0xdc00..=0xdfff).contains(&second) {
                                    return Err(
                                        "paire de substitution Unicode JSON invalide".into()
                                    );
                                }
                                0x10000
                                    + (((first - 0xd800) as u32) << 10)
                                    + (second - 0xdc00) as u32
                            } else if (0xdc00..=0xdfff).contains(&first) {
                                return Err("paire de substitution Unicode JSON invalide".into());
                            } else {
                                first as u32
                            };
                            result.push(char::from_u32(scalar).ok_or("caractère JSON invalide")?);
                        }
                        _ => return Err("échappement JSON invalide".into()),
                    }
                }
                0..=0x1f => return Err("caractère de contrôle dans une chaîne JSON".into()),
                _ => {
                    // A run of plain characters at once (linear in the length of the text).
                    let rest = &self.text[self.offset..];
                    let len = rest
                        .find(|c: char| c == '"' || c == '\\' || (c as u32) < 0x20)
                        .unwrap_or(rest.len());
                    result.push_str(&rest[..len]);
                    self.offset += len;
                }
            }
        }
    }

    fn hex_quad(&mut self) -> Result<u16, String> {
        let end = self.offset.checked_add(4).ok_or("JSON trop long")?;
        let bytes = self
            .input
            .get(self.offset..end)
            .ok_or("échappement Unicode JSON incomplet")?;
        let text = std::str::from_utf8(bytes).map_err(|_| "échappement Unicode JSON invalide")?;
        let value =
            u16::from_str_radix(text, 16).map_err(|_| "échappement Unicode JSON invalide")?;
        self.offset = end;
        Ok(value)
    }

    fn array(&mut self) -> Result<Json, String> {
        self.byte(b'[')?;
        let mut values = Vec::new();
        self.whitespace();
        if self.input.get(self.offset) == Some(&b']') {
            self.offset += 1;
            return Ok(Json::Array(values));
        }
        loop {
            values.push(self.value()?);
            self.whitespace();
            match self.input.get(self.offset) {
                Some(b',') => self.offset += 1,
                Some(b']') => {
                    self.offset += 1;
                    break;
                }
                _ => return Err(format!("tableau JSON invalide à l'octet {}", self.offset)),
            }
        }
        Ok(Json::Array(values))
    }

    fn object(&mut self) -> Result<Json, String> {
        self.byte(b'{')?;
        let mut fields = Vec::new();
        self.whitespace();
        if self.input.get(self.offset) == Some(&b'}') {
            self.offset += 1;
            return Ok(Json::Object(fields));
        }
        loop {
            self.whitespace();
            if self.input.get(self.offset) != Some(&b'"') {
                return Err(format!("clé JSON attendue à l'octet {}", self.offset));
            }
            let key = self.string()?;
            self.byte(b':')?;
            fields.push((key, self.value()?));
            self.whitespace();
            match self.input.get(self.offset) {
                Some(b',') => self.offset += 1,
                Some(b'}') => {
                    self.offset += 1;
                    break;
                }
                _ => return Err(format!("objet JSON invalide à l'octet {}", self.offset)),
            }
        }
        Ok(Json::Object(fields))
    }

    fn number(&mut self) -> Result<Json, String> {
        let start = self.offset;
        while self
            .input
            .get(self.offset)
            .is_some_and(|b| matches!(b, b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9'))
        {
            self.offset += 1;
        }
        let text =
            std::str::from_utf8(&self.input[start..self.offset]).map_err(|e| e.to_string())?;
        let value = text
            .parse::<f64>()
            .map_err(|_| format!("nombre JSON invalide à l'octet {start}"))?;
        Ok(Json::Number(value))
    }
}

fn parse(text: &str) -> Result<Json, String> {
    if text.len() > 1 << 20 {
        return Err("fichier de préférences trop volumineux".into());
    }
    let mut parser = Parser {
        text,
        input: text.as_bytes(),
        offset: 0,
        depth: 0,
    };
    let value = parser.value()?;
    parser.whitespace();
    if parser.offset != parser.input.len() {
        return Err(format!(
            "contenu après la valeur JSON à l'octet {}",
            parser.offset
        ));
    }
    Ok(value)
}

fn native_settings(old: &Json) -> Result<String, String> {
    let settings = old
        .get("settings")
        .ok_or("préférences de l'ancien Studio absentes")?;
    if !matches!(settings, Json::Object(_)) {
        return Err("préférences de l'ancien Studio invalides".into());
    }

    let boolean_or = |name: &str, fallback: bool| {
        settings
            .get(name)
            .and_then(Json::boolean)
            .unwrap_or(fallback)
    };
    let last_root = settings
        .get("lastRoot")
        .and_then(Json::string)
        .map(crate::http::json_str)
        .unwrap_or_else(|| "null".into());
    let theme = match settings
        .get("theme")
        .and_then(Json::string)
        .unwrap_or("dark")
    {
        "light" => "light",
        "system" => "system",
        _ => "dark",
    };
    let autosave = if boolean_or("autoSave", true) {
        "delay"
    } else {
        "off"
    };

    // One session per folder: roots that differ only in case keep the most recent (`t`).
    let mut sessions: Vec<(String, f64, String)> = Vec::new();
    if let Some(Json::Object(roots)) = old.get("roots") {
        for (root, state) in roots {
            let Some(session) = state.get("session") else {
                continue;
            };
            if !matches!(session, Json::Object(_)) {
                continue;
            }
            let open = session
                .get("open")
                .map(Json::strings)
                .into_iter()
                .flatten()
                .map(crate::http::json_str)
                .collect::<Vec<_>>()
                .join(",");
            let active = session
                .get("active")
                .and_then(Json::string)
                .map(crate::http::json_str)
                .unwrap_or_else(|| "null".into());
            let key = root.to_lowercase();
            let time = state.get("t").and_then(Json::number).unwrap_or(0.0);
            let entry = format!(
                "{}:{{\"open\":[{}],\"active\":{},\"expanded\":[]}}",
                crate::http::json_str(&key),
                open,
                active
            );
            match sessions.iter_mut().find(|(k, _, _)| *k == key) {
                Some(seen) if seen.1 >= time => {}
                Some(seen) => *seen = (key, time, entry),
                None => sessions.push((key, time, entry)),
            }
        }
    }
    let sessions: Vec<String> = sessions.into_iter().map(|(_, _, e)| e).collect();

    Ok(format!(
        "{{\"last_root\":{},\"theme\":{},\"sidebar\":{},\"panel\":{},\"minimal\":false,\"autosave\":{},\"sessions\":{{{}}}}}",
        last_root,
        crate::http::json_str(theme),
        boolean_or("sidebarOpen", true),
        boolean_or("panelOpen", true),
        crate::http::json_str(autosave),
        sessions.join(",")
    ))
}

/// A path that is a link or a junction (a reparse point that names another place).
fn is_link(path: &std::path::Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink())
}

/// Migrate the compatible settings once. Existing native settings always win.
/// The folder of the settings must really be in `%APPDATA%`: a link or a junction there
/// (a privileged installer would write through it) cancels the migration.
pub(crate) fn migrate_legacy_settings() -> Result<bool, String> {
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let dir = base.join("NEXUL Studio");
    let source = dir.join("state.json");
    let destination = dir.join("native.json");
    if destination.exists() || !source.is_file() || is_link(&dir) || is_link(&source) {
        return Ok(false);
    }
    let inside = match (fs::canonicalize(&base), fs::canonicalize(&dir)) {
        (Ok(b), Ok(d)) => d.starts_with(b),
        _ => false,
    };
    if !inside {
        return Ok(false);
    }

    let old = fs::read_to_string(&source)
        .map_err(|e| format!("lecture des préférences de l'ancien Studio : {e}"))?;
    let converted = native_settings(&parse(&old)?)?;
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temporary = dir.join(format!(".native-{}-{nonce}.tmp", std::process::id()));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|e| e.to_string())?;
    if let Err(error) = file.write_all(converted.as_bytes()) {
        drop(file);
        let _ = fs::remove_file(&temporary);
        return Err(error.to_string());
    }
    drop(file);
    match fs::hard_link(&temporary, &destination) {
        Ok(()) => {
            let _ = fs::remove_file(temporary);
            Ok(true)
        }
        Err(_) if destination.exists() => {
            let _ = fs::remove_file(temporary);
            Ok(false)
        }
        Err(error) => {
            let _ = fs::remove_file(temporary);
            Err(error.to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_preferences_convert_supported_settings_and_sessions() {
        let old = parse(r#"{
            "settings":{"theme":"light","lastRoot":"C:\\Projets\\Été","sidebarOpen":false,"panelOpen":true,"autoSave":false},
            "roots":{"C:\\Projets\\Été":{"session":{"open":["src\\main.nx"],"active":"src\\main.nx"},"t":7}}
        }"#).unwrap();
        let converted = native_settings(&old).unwrap();
        assert!(
            converted.contains("\"last_root\":\"C:\\\\Projets\\\\Été\""),
            "{converted}"
        );
        assert!(converted.contains("\"theme\":\"light\""), "{converted}");
        assert!(converted.contains("\"sidebar\":false"), "{converted}");
        assert!(converted.contains("\"panel\":true"), "{converted}");
        assert!(converted.contains("\"autosave\":\"off\""), "{converted}");
        assert!(converted.contains("\"c:\\\\projets\\\\été\":{\"open\":[\"src\\\\main.nx\"],\"active\":\"src\\\\main.nx\",\"expanded\":[]}"), "{converted}");
    }

    #[test]
    fn large_strings_parse_in_linear_time() {
        let long = "é".repeat(400_000);
        let text = format!("{{\"settings\":{{\"lastRoot\":\"{long}\\n\"}}}}");
        let start = std::time::Instant::now();
        let json = parse(&text).unwrap();
        assert!(start.elapsed() < std::time::Duration::from_secs(2), "{:?}", start.elapsed());
        let root = json.get("settings").and_then(|s| s.get("lastRoot")).and_then(Json::string).unwrap();
        assert_eq!(root.len(), long.len() + 1);
        assert!(root.ends_with('\n'));
    }

    #[test]
    fn folders_differing_only_in_case_keep_the_most_recent_session() {
        let old = parse(r#"{"settings":{},"roots":{
            "C:\\P":{"session":{"open":["new.nx"],"active":null},"t":9},
            "c:\\p":{"session":{"open":["old.nx"],"active":null},"t":3}
        }}"#)
        .unwrap();
        let converted = native_settings(&old).unwrap();
        assert!(converted.contains("new.nx") && !converted.contains("old.nx"), "{converted}");
        assert_eq!(converted.matches("\"c:\\\\p\"").count(), 1, "{converted}");
    }

    #[test]
    fn parser_rejects_incomplete_or_trailing_json() {
        assert!(parse(r#"{"settings": ""#).is_err());
        assert!(parse("{} trailing").is_err());
    }
}
