use rayon::prelude::*;
use serde::Serialize;
use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use walkdir::WalkDir;

/// Deep enough for the common `<root>/org/repo/crates/member/target` layout while
/// keeping accidental scans of huge directory trees bounded.
pub const MAX_SCAN_DEPTH: usize = 10;

const SKIPPED_DIRS: &[&str] = &[
    ".git",
    ".hg",
    ".svn",
    ".idea",
    ".vscode",
    "node_modules",
    "appdata",
    "application data",
    "$recycle.bin",
    "system volume information",
    "windows",
    "program files",
    "program files (x86)",
    "programdata",
];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetInfo {
    pub path: String,
    pub project_name: String,
    pub project_path: String,
    pub size_bytes: u64,
    pub file_count: u64,
    pub modified_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanProgress {
    /// `discovering`, `measuring` or `done`.
    pub stage: String,
    pub root: Option<String>,
    pub roots_scanned: usize,
    pub roots_total: usize,
    pub found: usize,
    pub measured: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanResult {
    pub targets: Vec<TargetInfo>,
    pub scanned_roots: usize,
    pub requested_roots: usize,
    pub skipped_roots: Vec<String>,
    pub scanned_dirs: u64,
    pub elapsed_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteFailure {
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteResult {
    pub deleted: Vec<String>,
    pub freed_bytes: u64,
    pub failures: Vec<DeleteFailure>,
    pub elapsed_ms: u64,
}

#[derive(Debug, Default)]
struct Measurement {
    size_bytes: u64,
    file_count: u64,
    modified_ms: Option<u64>,
}

/// Scan one or more roots for Cargo `target` directories.
///
/// Only directories named exactly `target` that sit next to a `Cargo.toml` are
/// considered. Directory symlinks are never followed.
pub fn scan_roots<F>(roots: Vec<String>, mut on_progress: F) -> ScanResult
where
    F: FnMut(ScanProgress),
{
    let started = Instant::now();
    let roots_total = roots.len();
    let mut scanned_roots = 0usize;
    let mut scanned_dirs = 0u64;
    let mut skipped_roots = Vec::new();
    let mut discovered: Vec<PathBuf> = Vec::new();
    let mut seen = HashSet::new();

    for (index, raw_root) in roots.iter().enumerate() {
        on_progress(ScanProgress {
            stage: "discovering".into(),
            root: Some(raw_root.clone()),
            roots_scanned: index,
            roots_total,
            found: discovered.len(),
            measured: 0,
        });

        let root = match fs::canonicalize(raw_root) {
            Ok(path) if path.is_dir() => path,
            _ => {
                skipped_roots.push(raw_root.clone());
                continue;
            }
        };

        scanned_roots += 1;
        let (candidates, dirs) = discover_targets(&root);
        scanned_dirs += dirs;

        for candidate in candidates {
            // Resolve `..` and duplicate roots, but never resolve a symlinked
            // target itself: `is_cargo_target` rejects those up front.
            let Ok(canonical) = fs::canonicalize(&candidate) else {
                continue;
            };
            if is_cargo_target(&canonical) && seen.insert(canonical.clone()) {
                discovered.push(canonical);
            }
        }
    }

    on_progress(ScanProgress {
        stage: "measuring".into(),
        root: None,
        roots_scanned: scanned_roots,
        roots_total,
        found: discovered.len(),
        measured: 0,
    });

    let mut targets: Vec<TargetInfo> = discovered
        .par_iter()
        .filter_map(|path| build_target_info(path))
        .collect();

    targets.sort_by(|a, b| {
        b.size_bytes
            .cmp(&a.size_bytes)
            .then_with(|| {
                a.project_name
                    .to_lowercase()
                    .cmp(&b.project_name.to_lowercase())
            })
            .then_with(|| a.path.cmp(&b.path))
    });

    on_progress(ScanProgress {
        stage: "done".into(),
        root: None,
        roots_scanned: scanned_roots,
        roots_total,
        found: targets.len(),
        measured: targets.len(),
    });

    ScanResult {
        targets,
        scanned_roots,
        requested_roots: roots_total,
        skipped_roots,
        scanned_dirs,
        elapsed_ms: started.elapsed().as_millis() as u64,
    }
}

/// Validate and delete Cargo target directories. Every path is re-validated
/// immediately before deletion, so a forged frontend payload cannot remove an
/// arbitrary directory.
pub fn delete_targets(paths: Vec<String>) -> DeleteResult {
    let started = Instant::now();
    let mut deleted = Vec::new();
    let mut failures = Vec::new();
    let mut freed_bytes = 0u64;
    let mut seen = HashSet::new();

    for raw_path in paths {
        let path = PathBuf::from(&raw_path);

        let canonical = match validate_target(&path) {
            Ok(path) => path,
            Err(message) => {
                failures.push(DeleteFailure {
                    path: raw_path,
                    message,
                });
                continue;
            }
        };

        if !seen.insert(canonical.clone()) {
            continue;
        }

        let size = measure_dir(&canonical).size_bytes;
        match remove_dir_all_robust(&canonical) {
            Ok(()) => {
                freed_bytes = freed_bytes.saturating_add(size);
                deleted.push(canonical.display().to_string());
            }
            Err(error) => failures.push(DeleteFailure {
                path: raw_path,
                message: format!("删除失败：{error}"),
            }),
        }
    }

    DeleteResult {
        deleted,
        freed_bytes,
        failures,
        elapsed_ms: started.elapsed().as_millis() as u64,
    }
}

/// Suggest existing, commonly used project folders on first launch.
pub fn default_scan_roots() -> Vec<String> {
    let mut candidates = Vec::new();

    if let Some(documents) = dirs::document_dir() {
        candidates.push(documents);
    }
    if let Some(desktop) = dirs::desktop_dir() {
        candidates.push(desktop);
    }
    if let Some(home) = dirs::home_dir() {
        for relative in [
            "source/repos",
            "source",
            "repos",
            "projects",
            "dev",
            "code",
            "workspace",
            "GitHub",
        ] {
            candidates.push(home.join(relative));
        }
    }

    let mut seen = HashSet::new();
    let mut roots = Vec::new();
    for candidate in candidates {
        if !candidate.is_dir() {
            continue;
        }
        let Ok(canonical) = fs::canonicalize(&candidate) else {
            continue;
        };
        if seen.insert(canonical.clone()) {
            roots.push(canonical.display().to_string());
        }
    }
    roots
}

fn discover_targets(root: &Path) -> (Vec<PathBuf>, u64) {
    let mut found = Vec::new();
    let mut visited_dirs = 0u64;

    if is_cargo_target(root) {
        found.push(root.to_path_buf());
        return (found, 1);
    }

    let mut walker = WalkDir::new(root)
        .max_depth(MAX_SCAN_DEPTH)
        .follow_links(false)
        .into_iter();

    while let Some(next) = walker.next() {
        let entry = match next {
            Ok(entry) => entry,
            Err(_) => continue,
        };

        if !entry.file_type().is_dir() {
            continue;
        }
        visited_dirs += 1;

        if entry.depth() == 0 {
            continue;
        }

        let name = entry.file_name().to_string_lossy();

        if name.eq_ignore_ascii_case("target") {
            walker.skip_current_dir();
            if is_cargo_target(entry.path()) {
                found.push(entry.path().to_path_buf());
            }
            continue;
        }

        if is_skipped_dir(&name) {
            walker.skip_current_dir();
        }
    }

    (found, visited_dirs)
}

fn is_cargo_target(path: &Path) -> bool {
    if path.file_name().and_then(|name| name.to_str()) != Some("target") {
        return false;
    }

    let Ok(metadata) = fs::symlink_metadata(path) else {
        return false;
    };
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return false;
    }

    path.parent()
        .map(|parent| parent.join("Cargo.toml").is_file())
        .unwrap_or(false)
}

fn is_skipped_dir(name: &str) -> bool {
    SKIPPED_DIRS
        .iter()
        .any(|skipped| name.eq_ignore_ascii_case(skipped))
}

fn build_target_info(path: &Path) -> Option<TargetInfo> {
    if !path.is_dir() {
        return None;
    }

    let parent = path.parent()?;
    let project_name = package_name(&parent.join("Cargo.toml")).unwrap_or_else(|| {
        parent
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("Rust project")
            .to_string()
    });
    let measurement = measure_dir(path);

    Some(TargetInfo {
        path: path.display().to_string(),
        project_name,
        project_path: parent.display().to_string(),
        size_bytes: measurement.size_bytes,
        file_count: measurement.file_count,
        modified_ms: measurement.modified_ms,
    })
}

fn package_name(cargo_toml: &Path) -> Option<String> {
    let contents = fs::read_to_string(cargo_toml).ok()?;
    let parsed: toml::Value = toml::from_str(&contents).ok()?;
    parsed
        .get("package")?
        .get("name")?
        .as_str()
        .map(ToOwned::to_owned)
}

fn measure_dir(path: &Path) -> Measurement {
    let mut measurement = Measurement::default();

    if let Ok(metadata) = fs::metadata(path) {
        measurement.modified_ms = metadata.modified().ok().and_then(system_time_ms);
    }

    let walker = WalkDir::new(path)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok);

    for entry in walker {
        if !entry.file_type().is_file() {
            continue;
        }
        let Ok(metadata) = entry.metadata() else {
            continue;
        };

        measurement.size_bytes = measurement.size_bytes.saturating_add(metadata.len());
        measurement.file_count += 1;

        if let Some(modified) = metadata.modified().ok().and_then(system_time_ms) {
            measurement.modified_ms = Some(measurement.modified_ms.unwrap_or(0).max(modified));
        }
    }

    measurement
}

fn system_time_ms(time: SystemTime) -> Option<u64> {
    time.duration_since(UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_millis() as u64)
}

fn validate_target(path: &Path) -> Result<PathBuf, String> {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return Err("路径无效，无法识别 target 目录。".into());
    };

    if name != "target" {
        return Err("出于安全考虑，只能删除名为 target 的目录。".into());
    }

    let metadata = fs::symlink_metadata(path).map_err(|_| "目录不存在或无法访问。".to_string())?;

    if metadata.file_type().is_symlink() {
        return Err("该 target 是符号链接，已跳过。".into());
    }
    if !metadata.is_dir() {
        return Err("目标路径不是目录。".into());
    }

    let parent = path
        .parent()
        .ok_or_else(|| "无法定位 target 的父目录。".to_string())?;
    if !parent.join("Cargo.toml").is_file() {
        return Err("父目录中没有 Cargo.toml，拒绝删除。".into());
    }

    let canonical = fs::canonicalize(path).map_err(|error| format!("无法解析路径：{error}"))?;

    if canonical.file_name().and_then(|name| name.to_str()) != Some("target") {
        return Err("路径解析后不是 target 目录，拒绝删除。".into());
    }

    let canonical_parent = canonical
        .parent()
        .ok_or_else(|| "无法定位 target 的父目录。".to_string())?;
    if !canonical_parent.join("Cargo.toml").is_file() {
        return Err("路径解析后未找到 Cargo.toml，拒绝删除。".into());
    }

    Ok(canonical)
}

fn remove_dir_all_robust(path: &Path) -> io::Result<()> {
    match fs::remove_dir_all(path) {
        Ok(()) => Ok(()),
        Err(first_error) => {
            #[cfg(windows)]
            {
                clear_readonly(path);
                fs::remove_dir_all(path).map_err(|second_error| {
                    io::Error::new(
                        second_error.kind(),
                        format!("{first_error}；清除只读属性后重试仍失败：{second_error}"),
                    )
                })
            }
            #[cfg(not(windows))]
            {
                Err(first_error)
            }
        }
    }
}

// This code is compiled on Windows only. Clearing the read-only attribute is
// exactly what we want there; the Unix warning does not apply.
#[cfg(windows)]
#[allow(clippy::permissions_set_readonly_false)]
fn clear_readonly(path: &Path) {
    for entry in WalkDir::new(path)
        .contents_first(true)
        .into_iter()
        .filter_map(Result::ok)
    {
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        if !metadata.permissions().readonly() {
            continue;
        }
        let mut permissions = metadata.permissions();
        permissions.set_readonly(false);
        let _ = fs::set_permissions(entry.path(), permissions);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn make_project(root: &Path, folder: &str, package: &str, bytes: usize) -> PathBuf {
        let project = root.join(folder);
        let target = project.join("target");
        fs::create_dir_all(target.join("debug")).unwrap();
        fs::write(
            project.join("Cargo.toml"),
            format!("[package]\nname = \"{package}\"\nversion = \"0.1.0\"\n"),
        )
        .unwrap();
        fs::write(target.join("debug").join("app.exe"), vec![7u8; bytes]).unwrap();
        project
    }

    #[test]
    fn finds_cargo_targets_and_measures_them() {
        let temp = tempdir().unwrap();
        let project = make_project(temp.path(), "demo", "demo-app", 2048);
        fs::create_dir_all(temp.path().join("not-rust").join("target")).unwrap();

        let result = scan_roots(vec![temp.path().display().to_string()], |_| {});

        assert_eq!(result.targets.len(), 1);
        let target = &result.targets[0];
        assert_eq!(target.project_name, "demo-app");
        assert_eq!(
            target.path,
            fs::canonicalize(project.join("target"))
                .unwrap()
                .display()
                .to_string()
        );
        assert!(target.size_bytes >= 2048);
        assert_eq!(target.file_count, 1);
    }

    #[test]
    fn does_not_descend_into_target_directories() {
        let temp = tempdir().unwrap();
        let first = make_project(temp.path(), "first", "first", 16);
        let nested = first.join("target").join("debug").join("nested");
        fs::create_dir_all(nested.join("target")).unwrap();
        fs::write(nested.join("Cargo.toml"), "[package]\nname = \"nested\"\n").unwrap();
        fs::write(nested.join("target").join("artifact"), b"data").unwrap();

        let result = scan_roots(vec![temp.path().display().to_string()], |_| {});

        assert_eq!(result.targets.len(), 1);
        assert_eq!(result.targets[0].project_name, "first");
    }

    #[test]
    fn deletion_rejects_a_target_without_a_cargo_manifest() {
        let temp = tempdir().unwrap();
        let target = temp.path().join("target");
        fs::create_dir_all(&target).unwrap();

        let result = delete_targets(vec![target.display().to_string()]);

        assert!(result.deleted.is_empty());
        assert_eq!(result.failures.len(), 1);
        assert!(target.exists());
    }

    #[test]
    fn deletion_removes_a_valid_cargo_target() {
        let temp = tempdir().unwrap();
        let project = make_project(temp.path(), "removable", "removable", 1024);
        let target = project.join("target");

        let result = delete_targets(vec![target.display().to_string()]);

        assert_eq!(result.deleted.len(), 1);
        assert_eq!(result.failures.len(), 0);
        assert!(!target.exists());
        assert!(project.join("Cargo.toml").exists());
    }

    #[test]
    fn rejects_a_path_that_is_not_named_target() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("src");
        fs::create_dir_all(&path).unwrap();

        assert!(validate_target(&path).is_err());
    }
}
