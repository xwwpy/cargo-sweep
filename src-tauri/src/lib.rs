mod cargo_source;
mod scanner;

use cargo_source::{ApplySourceResult, CargoSourceStatus};
use scanner::{DeleteResult, ScanProgress, ScanResult};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::Emitter;

fn startup_log(message: &str) {
    let base = dirs::data_local_dir().unwrap_or_else(std::env::temp_dir);
    let log_dir = base.join("CargoSweep").join("logs");
    if fs::create_dir_all(&log_dir).is_err() {
        return;
    }

    let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_dir.join("startup.log"))
    else {
        return;
    };

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default();
    let _ = writeln!(file, "[{timestamp}] {message}");
}

fn crash_log_path() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("CargoSweep")
        .join("logs")
        .join("startup.log")
}

/// Common Windows setups point TEMP at a folder that restricted processes
/// cannot use. Give WebView2 a private, known-writable runtime folder instead.
fn configure_runtime_temp() -> Option<PathBuf> {
    let base = dirs::data_local_dir()?;
    let runtime = base.join("CargoSweep").join("runtime");
    let temp = runtime.join("temp");
    let webview = runtime.join("webview");
    fs::create_dir_all(&temp).ok()?;
    fs::create_dir_all(&webview).ok()?;

    std::env::set_var("TEMP", &temp);
    std::env::set_var("TMP", &temp);
    std::env::set_var("WEBVIEW2_USER_DATA_FOLDER", &webview);
    Some(temp)
}

fn install_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        startup_log(&format!(
            "PANIC: {info}\ncrash log: {}\nTEMP={:?}\nLOCALAPPDATA={:?}",
            crash_log_path().display(),
            std::env::var("TEMP").ok(),
            std::env::var("LOCALAPPDATA").ok()
        ));
        default_hook(info);
    }));
}

#[tauri::command]
async fn scan_targets(app: tauri::AppHandle, roots: Vec<String>) -> Result<ScanResult, String> {
    if roots.len() > 64 {
        return Err("一次最多扫描 64 个根目录。".to_string());
    }

    tauri::async_runtime::spawn_blocking(move || {
        scanner::scan_roots(roots, |progress: ScanProgress| {
            let _ = app.emit("scan-progress", progress);
        })
    })
    .await
    .map_err(|error| format!("扫描任务异常退出：{error}"))
}

#[tauri::command]
async fn delete_targets(paths: Vec<String>) -> Result<DeleteResult, String> {
    if paths.is_empty() {
        return Err("没有选择要删除的 target 目录。".to_string());
    }
    if paths.len() > 512 {
        return Err("一次最多删除 512 个 target 目录。".to_string());
    }

    tauri::async_runtime::spawn_blocking(move || scanner::delete_targets(paths))
        .await
        .map_err(|error| format!("删除任务异常退出：{error}"))
}

#[tauri::command]
fn default_scan_roots() -> Vec<String> {
    scanner::default_scan_roots()
}

#[tauri::command]
fn detect_cargo_source() -> Result<CargoSourceStatus, String> {
    cargo_source::detect()
}

#[tauri::command]
async fn apply_cargo_source(url: String) -> Result<ApplySourceResult, String> {
    tauri::async_runtime::spawn_blocking(move || cargo_source::apply(url))
        .await
        .map_err(|error| format!("切换 Cargo 源任务异常退出：{error}"))?
}

#[tauri::command]
fn open_folder(path: String) -> Result<(), String> {
    let folder = std::path::PathBuf::from(&path);
    if !folder.is_dir() {
        return Err("目录不存在或无法访问。".to_string());
    }

    #[cfg(target_os = "windows")]
    let result = std::process::Command::new("explorer.exe")
        .arg(&folder)
        .spawn();

    #[cfg(target_os = "macos")]
    let result = std::process::Command::new("open").arg(&folder).spawn();

    #[cfg(all(unix, not(target_os = "macos")))]
    let result = std::process::Command::new("xdg-open").arg(&folder).spawn();

    result
        .map(|_| ())
        .map_err(|error| format!("无法打开目录：{error}"))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    install_panic_hook();
    let runtime_temp = configure_runtime_temp();
    startup_log(&format!(
        "Cargo Sweep starting; runtime temp={runtime_temp:?}"
    ));

    let result = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|_app| {
            startup_log("Tauri setup completed");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            scan_targets,
            delete_targets,
            default_scan_roots,
            detect_cargo_source,
            apply_cargo_source,
            open_folder
        ])
        .run(tauri::generate_context!());

    if let Err(error) = result {
        startup_log(&format!("Tauri run failed: {error}"));
        eprintln!("Cargo Sweep failed to start: {error}");
        std::process::exit(1);
    }
}
