use crate::i18n::Lang;
use std::path::PathBuf;

#[derive(serde::Serialize, serde::Deserialize)]
pub struct Prefs {
    pub lang: Lang,
    pub outline: bool,
    pub recent: Vec<PathBuf>,
}

impl Default for Prefs {
    fn default() -> Self {
        Prefs { lang: Lang::Zh, outline: true, recent: Vec::new() }
    }
}

fn path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("xmind-rs")
        .join("prefs.json")
}

impl Prefs {
    pub fn load() -> Prefs {
        std::fs::read(path())
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        let p = path();
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(s) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(p, s);
        }
    }
}
