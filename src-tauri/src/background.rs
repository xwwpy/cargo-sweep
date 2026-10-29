use crate::rust_toolchain::{ComponentProgress, OperationProgress};
use serde::Serialize;
use std::cell::RefCell;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

#[derive(Default)]
pub struct TaskControl {
    cancelled: AtomicBool,
    pid: AtomicU32,
}

impl TaskControl {
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }

    pub fn set_pid(&self, pid: u32) {
        self.pid.store(pid, Ordering::SeqCst);
    }

    pub fn clear_pid(&self) {
        self.pid.store(0, Ordering::SeqCst);
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
        let pid = self.pid.load(Ordering::SeqCst);
        if pid > 0 {
            kill_process_tree(pid);
        }
    }
}

struct Registry {
    tasks: Mutex<HashMap<String, Arc<TaskControl>>>,
    kinds: Mutex<HashMap<String, String>>,
}

static REGISTRY: OnceLock<Registry> = OnceLock::new();

fn registry() -> &'static Registry {
    REGISTRY.get_or_init(|| Registry {
        tasks: Mutex::new(HashMap::new()),
        kinds: Mutex::new(HashMap::new()),
    })
}

thread_local! {
    static CURRENT_TASK: RefCell<Option<Arc<TaskControl>>> = const { RefCell::new(None) };
}

pub fn register(task_id: &str, kind: &str) -> Arc<TaskControl> {
    let control = Arc::new(TaskControl::default());
    if let Ok(mut tasks) = registry().tasks.lock() {
        tasks.insert(task_id.to_string(), Arc::clone(&control));
    }
    if let Ok(mut kinds) = registry().kinds.lock() {
        kinds.insert(task_id.to_string(), kind.to_string());
    }
    control
}

pub fn unregister(task_id: &str) {
    if let Ok(mut tasks) = registry().tasks.lock() {
        tasks.remove(task_id);
    }
    if let Ok(mut kinds) = registry().kinds.lock() {
        kinds.remove(task_id);
    }
}

pub fn cancel(task_id: &str) -> bool {
    let control = registry()
        .tasks
        .lock()
        .ok()
        .and_then(|tasks| tasks.get(task_id).cloned());
    if let Some(control) = control {
        control.cancel();
        true
    } else {
        false
    }
}

pub fn with_task<T>(task_id: &str, run: impl FnOnce() -> T) -> T {
    let control = registry()
        .tasks
        .lock()
        .ok()
        .and_then(|tasks| tasks.get(task_id).cloned());
    CURRENT_TASK.with(|current| {
        *current.borrow_mut() = control;
    });
    let result = run();
    CURRENT_TASK.with(|current| {
        *current.borrow_mut() = None;
    });
    result
}

pub fn current_control() -> Option<Arc<TaskControl>> {
    CURRENT_TASK.with(|current| current.borrow().clone())
}

pub fn current_cancelled() -> bool {
    current_control()
        .map(|control| control.is_cancelled())
        .unwrap_or(false)
}

pub fn active_toolchain_tasks() -> bool {
    let kinds = registry().kinds.lock();
    let tasks = registry().tasks.lock();
    match (kinds, tasks) {
        (Ok(kinds), Ok(tasks)) => kinds
            .iter()
            .any(|(id, kind)| !kind.starts_with("delete") && tasks.contains_key(id)),
        _ => true,
    }
}

fn kill_process_tree(pid: u32) {
    #[cfg(windows)]
    {
        let _ = Command::new("taskkill")
            .args(["/F", "/T", "/PID", &pid.to_string()])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    #[cfg(unix)]
    {
        let _ = Command::new("kill")
            .args(["-TERM", &pid.to_string()])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskProgressEvent {
    pub task_id: String,
    pub percent: u8,
    pub message: String,
    pub phase: String,
    pub line: Option<String>,
    pub components: Vec<ComponentProgress>,
    pub speed_bps: u64,
    pub elapsed_ms: u64,
    pub downloaded_bytes: u64,
}

impl TaskProgressEvent {
    pub fn simple(
        task_id: String,
        percent: u8,
        message: impl Into<String>,
        phase: impl Into<String>,
    ) -> Self {
        Self {
            task_id,
            percent,
            message: message.into(),
            phase: phase.into(),
            line: None,
            components: Vec::new(),
            speed_bps: 0,
            elapsed_ms: 0,
            downloaded_bytes: 0,
        }
    }

    pub fn from_progress(task_id: String, progress: OperationProgress) -> Self {
        Self {
            task_id,
            percent: progress.percent,
            message: progress.message,
            phase: progress.phase,
            line: progress.line,
            components: progress.components,
            speed_bps: progress.speed_bps,
            elapsed_ms: progress.elapsed_ms,
            downloaded_bytes: progress.downloaded_bytes,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupReport {
    pub removed_files: u64,
    pub freed_bytes: u64,
    pub skipped: bool,
    pub message: String,
}

fn rustup_home() -> Option<PathBuf> {
    env::var_os("RUSTUP_HOME")
        .map(PathBuf::from)
        .or_else(|| dirs::home_dir().map(|home| home.join(".rustup")))
}

fn remove_artifact(path: &Path) -> (u64, u64) {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(_) => return (0, 0),
    };
    let freed = if metadata.is_file() {
        metadata.len()
    } else {
        0
    };
    for attempt in 0..3 {
        let removed = if metadata.file_type().is_symlink() {
            fs::remove_file(path).is_ok()
        } else if metadata.is_dir() {
            fs::remove_dir_all(path).is_ok()
        } else {
            fs::remove_file(path).is_ok()
        };
        if removed {
            return (1, freed);
        }
        if attempt < 2 {
            std::thread::sleep(std::time::Duration::from_millis(250));
        }
    }
    (0, 0)
}

pub fn cleanup_artifacts(_started_ms: u64, kind: &str) -> CleanupReport {
    if kind == "delete" {
        return CleanupReport {
            removed_files: 0,
            freed_bytes: 0,
            skipped: false,
            message: "空间清理任务没有下载缓存等中间文件".into(),
        };
    }
    if active_toolchain_tasks() {
        return CleanupReport {
            removed_files: 0,
            freed_bytes: 0,
            skipped: true,
            message: "仍有其他 Rust 任务在运行，已跳过临时文件清理".into(),
        };
    }
    let Some(home) = rustup_home() else {
        return CleanupReport {
            removed_files: 0,
            freed_bytes: 0,
            skipped: false,
            message: "未找到 RUSTUP_HOME，没有可清理的临时文件".into(),
        };
    };
    cleanup_artifacts_in(&home)
}

fn cleanup_artifacts_in(home: &Path) -> CleanupReport {
    let mut removed_files = 0;
    let mut freed_bytes = 0;
    for folder in ["downloads", "tmp"] {
        let directory = home.join(folder);
        let Ok(entries) = fs::read_dir(directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let (count, bytes) = remove_artifact(&entry.path());
            removed_files += count;
            freed_bytes += bytes;
        }
    }
    CleanupReport {
        removed_files,
        freed_bytes,
        skipped: false,
        message: if removed_files > 0 {
            "已清理本次任务留下的下载缓存和临时文件".into()
        } else {
            "没有发现需要清理的临时文件".into()
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process;

    #[test]
    fn cleanup_removes_all_download_and_temp_entries() {
        let root = std::env::temp_dir().join(format!(
            "cargo-sweep-cleanup-{}-{}",
            process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or_default()
        ));
        let downloads = root.join("downloads");
        let temporary = root.join("tmp");
        fs::create_dir_all(&downloads).unwrap();
        fs::create_dir_all(temporary.join("partial")).unwrap();
        fs::write(downloads.join("archive"), vec![0u8; 32]).unwrap();
        fs::write(temporary.join("partial").join("file"), b"partial").unwrap();

        let report = cleanup_artifacts_in(&root);

        assert!(report.removed_files >= 2);
        assert!(report.freed_bytes >= 32);
        assert!(!downloads.join("archive").exists());
        assert!(!temporary.join("partial").exists());
        fs::remove_dir_all(root).unwrap();
    }
}
