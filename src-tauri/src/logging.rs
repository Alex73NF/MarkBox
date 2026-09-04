//! 会话日志：GUI 子系统下 stderr 在 release 被 std 静默丢弃，错误与 panic 落盘 markbox.log

use std::fmt::Display;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::Manager;

/// 本会话日志文件路径；None = 文件日志不可用（配置目录不可写等），仅剩 stderr
static LOG_FILE: Mutex<Option<PathBuf>> = Mutex::new(None);

/// 初始化会话日志：登记路径、截断上次会话内容，并装 panic 钩子。
/// GUI 子系统（windows_subsystem="windows"）没有控制台，stderr 写入在 release 下被 std
/// 静默丢弃（映射 EBADF 为成功），排障只能依赖这份文件。
pub(crate) fn init(app: &tauri::AppHandle) {
    let path = app
        .path()
        .app_log_dir()
        .or_else(|_| app.path().app_config_dir())
        .ok()
        .map(|dir| dir.join("markbox.log"));
    if let Some(p) = &path {
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        // 每次启动截断：错误与 panic 极少，单会话体量微小，免去轮转机制
        let _ = std::fs::File::create(p);
    }
    *LOG_FILE.lock().unwrap() = path;
    std::panic::set_hook(Box::new(|info| log_error(&info.to_string())));
}

/// 唯一日志出口：写 stderr（dev/调试可见）并 best-effort 追加到会话日志文件，自身绝不失败
pub(crate) fn log_error(message: &str) {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let line = format!("[markbox] [{secs}] {message}");
    // 不用 eprintln!：其写失败会 panic，panic hook 内递归即 abort（dev 下管道断裂 EPIPE 可达）
    let _ = writeln!(std::io::stderr(), "{line}");
    // 临界区只含 Option<PathBuf>::clone（不可 panic），毒化不可达；防御性容忍以免 hook 内二次 panic
    let path = LOG_FILE.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).clone();
    if let Some(p) = path {
        if let Ok(mut f) = std::fs::OpenOptions::new().append(true).create(true).open(p) {
            let _ = writeln!(f, "{line}");
        }
    }
}

/// best-effort 操作的统一错误出口：失败记日志，成功静默。
/// 仓库内全部"失败仅记日志"的操作都是 Result<(), E>，返回值无消费方
pub(crate) fn log_err<E: Display>(context: &str, result: Result<(), E>) {
    if let Err(e) = result {
        log_error(&format!("{context}: {e}"));
    }
}
