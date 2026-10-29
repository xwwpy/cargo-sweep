mod background;
mod cargo_home;
mod cargo_source;
mod rust_toolchain;
mod rustup_source;
mod scanner;
mod speed_test;

use background::{CleanupReport, TaskProgressEvent};
use cargo_home::{CargoHomeMigrationResult, CargoHomeStatus};
use cargo_source::{ApplySourceResult, CargoSourceStatus};
use rust_toolchain::{
    LatestRustVersions, RustInstallationProbe, RustToolchainStatus, ToolchainOperationResult,
};
use rustup_source::RustupSourceStatus;
use scanner::{DeleteResult, ScanProgress, ScanResult};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
#[cfg(target_os = "macos")]
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::{Emitter, Manager};

#[derive(Default)]
struct OperationState {
    active: AtomicBool,
}

#[tauri::command]
fn show_main_window(app: tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

#[tauri::command]
fn log_frontend_error(message: String) {
    startup_log(&format!("FRONTEND ERROR: {message}"));
}

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

/// Some Windows setups point TEMP at a folder that restricted processes
/// cannot use. Give WebView2 a private, known-writable runtime folder instead.
/// The same private folder is also used on macOS and Linux via TMPDIR.
fn configure_runtime_temp() -> Option<PathBuf> {
    let base = dirs::data_local_dir()?;
    let runtime = base.join("CargoSweep").join("runtime");
    let temp = runtime.join("temp");
    let webview = runtime.join("webview");
    fs::create_dir_all(&temp).ok()?;
    fs::create_dir_all(&webview).ok()?;

    std::env::set_var("TEMP", &temp);
    std::env::set_var("TMP", &temp);
    std::env::set_var("TMPDIR", &temp);
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
async fn delete_targets(
    app: tauri::AppHandle,
    task_id: String,
    paths: Vec<String>,
) -> Result<DeleteResult, String> {
    if paths.is_empty() {
        return Err("没有选择要删除的 target 目录。".to_string());
    }
    if paths.len() > 512 {
        return Err("一次最多删除 512 个 target 目录。".to_string());
    }

    background::register(&task_id, "delete");
    let progress_task_id = task_id.clone();
    let cleanup_task_id = task_id.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        background::with_task(&task_id, || {
            scanner::delete_targets_with_progress(
                paths,
                background::current_cancelled,
                |index, total, path| {
                    let percent = ((index * 100) / total.max(1)) as u8;
                    let name = std::path::Path::new(path)
                        .file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or("target");
                    let _ = app.emit(
                        "task-progress",
                        TaskProgressEvent::simple(
                            progress_task_id.clone(),
                            percent,
                            format!("正在清理 {}/{}：{name}", index + 1, total),
                            "delete",
                        ),
                    );
                },
            )
        })
    })
    .await
    .map_err(|error| format!("删除任务异常退出：{error}"));
    background::unregister(&cleanup_task_id);
    result
}

#[tauri::command]
async fn cancel_background_task(task_id: String) -> Result<bool, String> {
    tauri::async_runtime::spawn_blocking(move || background::cancel(&task_id))
        .await
        .map_err(|error| format!("取消任务异常退出：{error}"))
}

#[tauri::command]
async fn cleanup_task_artifacts(started_at_ms: u64, kind: String) -> Result<CleanupReport, String> {
    tauri::async_runtime::spawn_blocking(move || {
        background::cleanup_artifacts(started_at_ms, &kind)
    })
    .await
    .map_err(|error| format!("清理临时文件异常退出：{error}"))
}

#[tauri::command]
async fn test_source_speed(
    url: String,
    max_bytes: Option<u64>,
) -> Result<speed_test::SourceSpeedResult, String> {
    tauri::async_runtime::spawn_blocking(move || speed_test::test_source(&url, max_bytes))
        .await
        .map_err(|error| format!("测速任务异常退出：{error}"))
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
fn cargo_home_status() -> Result<CargoHomeStatus, String> {
    cargo_home::status()
}

#[tauri::command]
fn rustup_source_status() -> Result<RustupSourceStatus, String> {
    rustup_source::status()
}

#[tauri::command]
async fn apply_rustup_source(
    source_id: String,
    dist_server: Option<String>,
    update_root: Option<String>,
) -> Result<RustupSourceStatus, String> {
    tauri::async_runtime::spawn_blocking(move || {
        rustup_source::apply(source_id, dist_server, update_root)
    })
    .await
    .map_err(|error| format!("切换 Rustup 下载源任务异常退出：{error}"))?
}

#[tauri::command]
async fn migrate_cargo_home(
    target: String,
    create_link: bool,
) -> Result<CargoHomeMigrationResult, String> {
    tauri::async_runtime::spawn_blocking(move || cargo_home::migrate(target, create_link))
        .await
        .map_err(|error| format!("迁移 CARGO_HOME 任务异常退出：{error}"))?
}

#[tauri::command]
async fn migrate_rustup_home(
    target: String,
    create_link: bool,
) -> Result<CargoHomeMigrationResult, String> {
    tauri::async_runtime::spawn_blocking(move || cargo_home::migrate_rustup(target, create_link))
        .await
        .map_err(|error| format!("迁移 RUSTUP_HOME 任务异常退出：{error}"))?
}

#[tauri::command]
async fn set_home_path(kind: String, target: String) -> Result<CargoHomeStatus, String> {
    tauri::async_runtime::spawn_blocking(move || cargo_home::set_home_path(&kind, target))
        .await
        .map_err(|error| format!("更新存储位置任务异常退出：{error}"))?
}

#[tauri::command]
fn probe_rust_installation() -> RustInstallationProbe {
    rust_toolchain::probe()
}

#[tauri::command]
async fn detect_rust_toolchain() -> Result<RustToolchainStatus, String> {
    tauri::async_runtime::spawn_blocking(rust_toolchain::detect)
        .await
        .map_err(|error| format!("检测 Rust 工具链任务异常退出：{error}"))?
}

#[tauri::command]
async fn check_rust_toolchain_latest() -> Result<LatestRustVersions, String> {
    tauri::async_runtime::spawn_blocking(rust_toolchain::latest)
        .await
        .map_err(|error| format!("检查 Rust 最新版本任务异常退出：{error}"))?
}

#[tauri::command]
async fn set_rustup_path(path: String) -> Result<RustToolchainStatus, String> {
    tauri::async_runtime::spawn_blocking(move || rust_toolchain::set_rustup_path(path))
        .await
        .map_err(|error| format!("设置 rustup 路径任务异常退出：{error}"))?
}

fn emit_toolchain_progress(
    app: &tauri::AppHandle,
    task_id: &str,
    progress: rust_toolchain::OperationProgress,
) {
    let _ = app.emit("toolchain-progress", progress.clone());
    let _ = app.emit(
        "task-progress",
        TaskProgressEvent::from_progress(task_id.to_string(), progress),
    );
}

#[tauri::command]
async fn install_rustup(
    app: tauri::AppHandle,
    task_id: String,
) -> Result<ToolchainOperationResult, String> {
    background::register(&task_id, "rustup");
    let progress_task_id = task_id.clone();
    let cleanup_task_id = task_id.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        background::with_task(&task_id, || {
            rust_toolchain::install_rustup_with_progress(|progress| {
                emit_toolchain_progress(&app, &progress_task_id, progress);
            })
        })
    })
    .await
    .map_err(|error| format!("安装 rustup 任务异常退出：{error}"));
    background::unregister(&cleanup_task_id);
    result?
}

#[tauri::command]
fn open_privacy_settings() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_AllFiles")
            .spawn()
            .map(|_| ())
            .map_err(|error| format!("无法打开系统隐私设置：{error}"))
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err("当前系统无需额外的完全磁盘访问权限。".into())
    }
}

#[tauri::command]
async fn install_rust_toolchain(
    app: tauri::AppHandle,
    task_id: String,
    target: String,
    set_default: bool,
) -> Result<ToolchainOperationResult, String> {
    background::register(&task_id, "toolchain");
    let progress_task_id = task_id.clone();
    let cleanup_task_id = task_id.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        background::with_task(&task_id, || {
            rust_toolchain::install_with_progress(target, set_default, |progress| {
                emit_toolchain_progress(&app, &progress_task_id, progress);
            })
        })
    })
    .await
    .map_err(|error| format!("安装 Rust 工具链任务异常退出：{error}"));
    background::unregister(&cleanup_task_id);
    result?
}

#[tauri::command]
async fn set_default_rust_toolchain(target: String) -> Result<ToolchainOperationResult, String> {
    tauri::async_runtime::spawn_blocking(move || rust_toolchain::set_default(target))
        .await
        .map_err(|error| format!("切换 Rust 工具链任务异常退出：{error}"))?
}

#[tauri::command]
async fn update_rust_toolchain(
    app: tauri::AppHandle,
    task_id: String,
    target: Option<String>,
) -> Result<ToolchainOperationResult, String> {
    background::register(&task_id, "toolchain");
    let progress_task_id = task_id.clone();
    let cleanup_task_id = task_id.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        background::with_task(&task_id, || {
            rust_toolchain::update_with_progress(target, |progress| {
                emit_toolchain_progress(&app, &progress_task_id, progress);
            })
        })
    })
    .await
    .map_err(|error| format!("更新 Rust 工具链任务异常退出：{error}"));
    background::unregister(&cleanup_task_id);
    result?
}

#[tauri::command]
async fn uninstall_rust_toolchain(
    app: tauri::AppHandle,
    task_id: String,
    target: String,
) -> Result<ToolchainOperationResult, String> {
    background::register(&task_id, "toolchain");
    let blocking_task_id = task_id.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        background::with_task(&blocking_task_id, || rust_toolchain::uninstall(target))
    })
    .await
    .map_err(|error| format!("删除 Rust 工具链任务异常退出：{error}"));
    background::unregister(&task_id);
    if let Ok(Ok(value)) = &result {
        let _ = app.emit(
            "task-progress",
            TaskProgressEvent::simple(
                task_id,
                if value.success { 100 } else { 0 },
                if value.success {
                    "工具链已删除".to_string()
                } else {
                    "删除工具链失败".to_string()
                },
                "delete",
            ),
        );
    }
    result?
}

#[tauri::command]
fn set_operation_active(state: tauri::State<OperationState>, active: bool) {
    state.active.store(active, Ordering::SeqCst);
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

    let app = match tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(OperationState::default())
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window
                    .state::<OperationState>()
                    .active
                    .load(Ordering::SeqCst)
                {
                    api.prevent_close();
                    let _ = window.emit("operation-close-blocked", ());
                }
            }
        })
        .setup(|app| {
            if let Some(window) = app.get_webview_window("main") {
                let fallback = window.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(Duration::from_millis(2500));
                    if !fallback.is_visible().unwrap_or(false) {
                        startup_log("Showing main window from startup fallback");
                        let _ = fallback.show();
                        let _ = fallback.set_focus();
                    }
                });
            }
            startup_log("Tauri setup completed");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            scan_targets,
            delete_targets,
            cancel_background_task,
            cleanup_task_artifacts,
            test_source_speed,
            show_main_window,
            log_frontend_error,
            default_scan_roots,
            cargo_home_status,
            rustup_source_status,
            apply_rustup_source,
            migrate_cargo_home,
            migrate_rustup_home,
            set_home_path,
            detect_cargo_source,
            apply_cargo_source,
            probe_rust_installation,
            detect_rust_toolchain,
            check_rust_toolchain_latest,
            set_rustup_path,
            install_rustup,
            open_privacy_settings,
            install_rust_toolchain,
            set_default_rust_toolchain,
            update_rust_toolchain,
            uninstall_rust_toolchain,
            set_operation_active,
            open_folder
        ])
        .build(tauri::generate_context!())
    {
        Ok(app) => app,
        Err(error) => {
            startup_log(&format!("Tauri run failed: {error}"));
            eprintln!("Cargo Sweep failed to start: {error}");
            std::process::exit(1);
        }
    };

    app.run(|app_handle, event| {
        if let tauri::RunEvent::ExitRequested { api, .. } = event {
            if app_handle
                .state::<OperationState>()
                .active
                .load(Ordering::SeqCst)
            {
                api.prevent_exit();
                let _ = app_handle.emit("operation-close-blocked", ());
            }
        }
    });
}
