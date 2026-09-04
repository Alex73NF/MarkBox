//! 设置持久化：JSON 原子读写（tmp+改名+刷盘）、归一化校验与崩溃残留清理

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

/// 并发 save_to 的临时文件序号：固定 tmp 名会让两个并发写互相踩踏
static TMP_SEQ: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct Settings {
    pub border_color: String,
    // i32 而非 u8/u16：手改 JSON 写入 300 或 -5 这类超范围值时先反序列化成功、再由 normalize 钳制，
    // 而不是无符号窄类型解析失败导致整个文件回退默认
    pub border_width: i32,
    pub border_radius: i32,
}

impl Default for Settings {
    fn default() -> Self {
        Self { border_color: "#FF4D4F".into(), border_width: 3, border_radius: 0 }
    }
}

/// 颜色格式非法回退默认；粗细钳 1-10、圆角钳 0-16
pub(crate) fn normalize(s: &Settings) -> Settings {
    let color_ok = s.border_color.len() == 7
        && s.border_color.starts_with('#')
        && s.border_color[1..].chars().all(|c| c.is_ascii_hexdigit());
    Settings {
        border_color: if color_ok { s.border_color.clone() } else { "#FF4D4F".into() },
        border_width: s.border_width.clamp(1, 10),
        border_radius: s.border_radius.clamp(0, 16),
    }
}

/// 配置文件路径（app_config_dir 由固定 identifier 派生，实际不会为 None）
pub(crate) fn settings_path(app: &AppHandle) -> PathBuf {
    app.path().app_config_dir().expect("app_config_dir must be valid").join("settings.json")
}

/// 从磁盘加载设置。返回 (设置, 是否需要重建)：文件缺失或损坏时回退默认值并由调用方重建；
/// 正常路径不动磁盘（归一化只作用于内存态）
pub(crate) fn load_or_repair(path: &Path) -> (Settings, bool) {
    match std::fs::read_to_string(path).ok().and_then(|txt| serde_json::from_str::<Settings>(&txt).ok()) {
        Some(s) => (normalize(&s), false),
        None => (Settings::default(), true),
    }
}

pub(crate) fn save_to(path: &Path, s: &Settings) -> std::io::Result<()> {
    use std::io::Write;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    // 先写临时文件再原子改名：写入中途崩溃也不会留下半个 settings.json；
    // tmp 名带 pid + 序号，防并发 save_to（如防抖保存与颜色即时保存交叠）写同一 tmp 互相覆盖
    let tmp = path.with_extension(format!(
        "json.tmp-{}-{}",
        std::process::id(),
        TMP_SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    let write = || -> std::io::Result<()> {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(serde_json::to_string_pretty(s).expect("settings serialize").as_bytes())?;
        // 改名前先刷盘：rename 只保证进程崩溃安全，断电/系统崩溃时改名元数据可能先于数据块落盘
        f.sync_all()?;
        drop(f);
        std::fs::rename(&tmp, path)
    };
    let result = write();
    if result.is_err() {
        // 失败路径清掉 tmp，不给"启动清理崩溃残留"多留一类来源（磁盘满时更不该累积）
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

/// 清理崩溃残留的临时文件：save_to 崩在写盘与改名之间会留下 settings.json.tmp-*（唯一名不再自覆盖），
/// 启动时 best-effort 清掉；此时没有任何在途保存，也不会误删
pub(crate) fn cleanup_tmp_leftovers(settings_path: &Path) {
    let Some(dir) = settings_path.parent() else { return };
    if !dir.is_dir() {
        return; // 全新安装：配置目录尚不存在，无残留可清
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        crate::logging::log_error("读取配置目录失败，跳过残留临时文件清理");
        return;
    };
    for entry in entries.flatten() {
        if entry.file_name().to_string_lossy().starts_with("settings.json.tmp-") {
            crate::logging::log_err(
                &format!("清理残留临时文件 {} 失败", entry.path().display()),
                std::fs::remove_file(entry.path()),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_path(name: &str) -> PathBuf {
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
    fn normalize_clamps_boundaries() {
        let lo = normalize(&Settings { border_color: "#FF4D4F".into(), border_width: 0, border_radius: 0 });
        assert_eq!(lo.border_width, 1);
        // 300 曾是 u8 反序列化失败值，现在应能读入并钳到上限
        let hi = normalize(&Settings { border_color: "#FF4D4F".into(), border_width: 300, border_radius: 300 });
        assert_eq!(hi.border_width, 10);
        assert_eq!(hi.border_radius, 16);
        // 负值同样先读入再钳制（i32 化的意义），而不是反序列化失败回退默认
        let neg = normalize(&Settings { border_color: "#FF4D4F".into(), border_width: -5, border_radius: -1 });
        assert_eq!(neg.border_width, 1);
        assert_eq!(neg.border_radius, 0);
    }

    #[test]
    fn color_validation() {
        // 小写 6 位十六进制合法
        let lower = normalize(&Settings { border_color: "#ff4d4f".into(), border_width: 3, border_radius: 0 });
        assert_eq!(lower.border_color, "#ff4d4f");
        // "#aaaaé"：é 占 2 字节，总长恰 7；非十六进制 → 回退默认。同时钉住 && 求值顺序——
        // 若把 starts_with('#') 挪到 [1..] 切片之后，此例会 panic 而非回退
        for bad in ["#FFF", "#GGGGGG", "#aaaaé"] {
            let s = normalize(&Settings { border_color: bad.into(), border_width: 3, border_radius: 0 });
            assert_eq!(s.border_color, "#FF4D4F");
        }
    }

    #[test]
    fn save_then_load_roundtrip() {
        let p = tmp_path("roundtrip");
        let s = Settings { border_color: "#00C2FF".into(), border_width: 5, border_radius: 8 };
        save_to(&p, &s).unwrap();
        assert_eq!(load_or_repair(&p), (s.clone(), false));
        // 序列化方向必须是 camelCase（前端 types.ts 契约）：roundtrip 对改名不敏感，单靠它钉不住
        assert!(serde_json::to_string(&s).unwrap().contains(r#""borderColor""#));
        // 成功路径不残留 tmp（rename 已消费）；并行测试各用各的文件名前缀，互不干扰
        let prefix = format!("{}.tmp-", p.file_name().unwrap().to_string_lossy());
        let leftovers = std::fs::read_dir(p.parent().unwrap())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().starts_with(&prefix))
            .count();
        assert_eq!(leftovers, 0);
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn load_missing_or_broken_file_falls_back_to_default() {
        assert_eq!(load_or_repair(Path::new("/nonexistent/markbox/settings.json")), (Settings::default(), true));
        let p = tmp_path("broken");
        std::fs::write(&p, "{ not json").unwrap();
        assert_eq!(load_or_repair(&p), (Settings::default(), true));
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn empty_object_parses_field_defaults() {
        // 容器级 #[serde(default)]：缺字段的合法 JSON 逐字段取默认，而不是解析失败回退
        let p = tmp_path("empty");
        std::fs::write(&p, "{}").unwrap();
        assert_eq!(load_or_repair(&p), (Settings::default(), false));
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn out_of_range_width_keeps_valid_color() {
        // 手改 borderWidth: 300 不应再连累合法的 borderColor（u8 时代整文件回退）
        let p = tmp_path("overflow");
        std::fs::write(&p, r##"{ "borderColor": "#00C2FF", "borderWidth": 300, "borderRadius": 0 }"##).unwrap();
        let (s, needs_repair) = load_or_repair(&p);
        assert_eq!(s.border_color, "#00C2FF");
        assert_eq!(s.border_width, 10);
        assert!(!needs_repair);
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn negative_width_parses_and_clamps_instead_of_repair() {
        // 负值同样不该触发整文件回退（i32 + normalize 钳制）
        let p = tmp_path("negative");
        std::fs::write(&p, r##"{ "borderColor": "#00C2FF", "borderWidth": -5, "borderRadius": 0 }"##).unwrap();
        let (s, needs_repair) = load_or_repair(&p);
        assert_eq!(s.border_color, "#00C2FF");
        assert_eq!(s.border_width, 1);
        assert!(!needs_repair);
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn cleanup_removes_stale_tmp_leftovers_only() {
        let dir = std::env::temp_dir().join(format!("markbox-test-cleanup-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let real = dir.join("settings.json");
        std::fs::write(&real, "{}").unwrap();
        let stale = dir.join("settings.json.tmp-424242-7");
        std::fs::write(&stale, "half written").unwrap();
        let foreign = dir.join("settings.json.bak"); // 非 tmp 命名的一律不动
        std::fs::write(&foreign, "keep").unwrap();
        cleanup_tmp_leftovers(&real);
        assert!(real.exists());
        assert!(!stale.exists());
        assert!(foreign.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
