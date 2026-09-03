use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub border_color: String,
    pub border_width: u8,
    pub border_radius: u8,
}

impl Default for Settings {
    fn default() -> Self {
        Self { border_color: "#FF4D4F".into(), border_width: 3, border_radius: 0 }
    }
}

/// 非法值回退默认（颜色格式、粗细 1-10、圆角 0-16）
pub fn normalize(s: &Settings) -> Settings {
    let color_ok = s.border_color.len() == 7
        && s.border_color.starts_with('#')
        && s.border_color[1..].chars().all(|c| c.is_ascii_hexdigit());
    Settings {
        border_color: if color_ok { s.border_color.clone() } else { "#FF4D4F".into() },
        border_width: s.border_width.clamp(1, 10),
        border_radius: s.border_radius.clamp(0, 16),
    }
}

pub fn load_from(path: &Path) -> Settings {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|txt| serde_json::from_str::<Settings>(&txt).ok())
        .map(|s| normalize(&s))
        .unwrap_or_default()
}

pub fn save_to(path: &Path, s: &Settings) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, serde_json::to_string_pretty(s).unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_path(name: &str) -> std::path::PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("markbox-test-{}-{}.json", name, std::process::id()));
        let _ = std::fs::remove_file(&p);
        p
    }

    #[test]
    fn default_values() {
        let s = Settings::default();
        assert_eq!(s.border_color, "#FF4D4F");
        assert_eq!(s.border_width, 3);
        assert_eq!(s.border_radius, 0);
    }

    #[test]
    fn normalize_fixes_invalid() {
        let s = normalize(&Settings { border_color: "red".into(), border_width: 99, border_radius: 99 });
        assert_eq!(s.border_color, "#FF4D4F");
        assert_eq!(s.border_width, 10);
        assert_eq!(s.border_radius, 16);
    }

    #[test]
    fn save_then_load_roundtrip() {
        let p = tmp_path("roundtrip");
        let s = Settings { border_color: "#00C2FF".into(), border_width: 5, border_radius: 8 };
        save_to(&p, &s).unwrap();
        assert_eq!(load_from(&p), s);
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn load_missing_or_broken_file_falls_back_to_default() {
        assert_eq!(load_from(Path::new("/nonexistent/markbox/settings.json")), Settings::default());
        let p = tmp_path("broken");
        std::fs::write(&p, "{ not json").unwrap();
        assert_eq!(load_from(&p), Settings::default());
        let _ = std::fs::remove_file(&p);
    }
}
