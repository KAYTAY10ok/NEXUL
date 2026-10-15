//! Settings and sessions of the native interface, kept in
//! `%APPDATA%\NEXUL Studio\native.json`.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// The files open in a folder, to reopen them.
#[derive(Serialize, Deserialize, Default, Clone, Debug)]
#[serde(default)]
pub struct Session {
    pub open: Vec<String>,
    pub active: Option<String>,
    pub expanded: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Settings {
    /// The folder open last.
    pub last_root: Option<String>,
    /// `dark`, `light` or `system`.
    pub theme: String,
    pub sidebar: bool,
    pub panel: bool,
    /// Code and interface: everything but the code is hidden.
    pub minimal: bool,
    /// Automatic saving: `off` (the default: Studio asks before losing anything) or
    /// `delay` (one second after the last change).
    pub autosave: String,
    pub sessions: HashMap<String, Session>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { last_root: None, theme: "dark".into(), sidebar: true, panel: false, minimal: false, autosave: "off".into(), sessions: HashMap::new() }
    }
}

/// `NEXUL_STUDIO_CONFIG` (a folder) replaces `%APPDATA%\NEXUL Studio` (tests).
pub fn dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("NEXUL_STUDIO_CONFIG") {
        return PathBuf::from(dir);
    }
    let base = std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    base.join("NEXUL Studio")
}

fn file() -> PathBuf {
    dir().join("native.json")
}

impl Settings {
    pub fn load() -> Settings {
        std::fs::read_to_string(file()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
    }

    /// Written through a temporary file, so that a crash never leaves half a file.
    pub fn save(&self) {
        let path = file();
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(text) = serde_json::to_string_pretty(self) {
            let tmp = path.with_extension("json.tmp");
            if std::fs::write(&tmp, text).is_ok() {
                let _ = std::fs::rename(&tmp, &path);
            }
        }
    }

    /// The session of a folder (paths compared without case, as on Windows).
    pub fn session(&mut self, root: &str) -> &mut Session {
        self.sessions.entry(root.to_lowercase()).or_default()
    }
}
