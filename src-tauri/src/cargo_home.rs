use serde::Serialize;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
#[cfg(windows)]
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};
use walkdir::WalkDir;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CargoHomeStatus {
    pub current: String,
    pub default: String,
    pub source: String,
    pub cargo_bin: String,
    pub rustup_home: String,
    pub path_contains_cargo_bin: bool,
    pub link_exists: bool,
    pub link_target: Option<String>,
    pub link_matches: bool,
    pub shell_profile: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CargoHomeMigrationResult {
    pub output: String,
    pub status: CargoHomeStatus,
}

pub fn status() -> Result<CargoHomeStatus, String> {
    let default = default_cargo_home()?;
    let env_home = env::var_os("CARGO_HOME").map(PathBuf::from);
    let saved_home = if env_home.is_none() {
        saved_cargo_home()
    } else {
        None
    };
    let current = env_home
        .clone()
        .or_else(|| saved_home.clone())
        .unwrap_or_else(|| default.clone());
    if saved_home.is_some() {
        env::set_var("CARGO_HOME", &current);
        prepend_path(&current.join("bin"));
    }
    let cargo_bin = current.join("bin");
    let rustup_home = rustup_home()?;
    let (link_exists, link_target) = link_state(&default);
    let link_matches = link_exists
        && canonical(&default)
            .zip(canonical(&current))
            .map(|(left, right)| left == right)
            .unwrap_or(false);

    Ok(CargoHomeStatus {
        current: display(&current),
        default: display(&default),
        source: if env_home.is_some() {
            "environment".into()
        } else if saved_home.is_some() {
            "app".into()
        } else {
            "default".into()
        },
        cargo_bin: display(&cargo_bin),
        rustup_home: display(&rustup_home),
        path_contains_cargo_bin: path_contains(&cargo_bin),
        link_exists,
        link_target: link_target.map(|path| display(&path)),
        link_matches,
        shell_profile: shell_profile().map(|path| display(&path)),
    })
}

pub fn migrate(target: String, create_link: bool) -> Result<CargoHomeMigrationResult, String> {
    let before = status()?;
    let default = default_cargo_home()?;
    let current = PathBuf::from(&before.current);
    if is_link(&current) {
        return Err("当前 CARGO_HOME 已经是一个链接，无需再次迁移。".into());
    }

    let target = expand_tilde(&target);
    if !target.is_absolute() {
        return Err("请选择绝对路径作为新的 CARGO_HOME。".into());
    }
    if same_path(&target, &current) {
        return Err("新目录与当前 CARGO_HOME 相同。".into());
    }
    if same_path(&target, &default) {
        return Err("新目录不能是默认的 ~/.cargo。".into());
    }

    let source = canonical(&current).unwrap_or(current.clone());
    let target = canonical_or_self(&target);
    if target.starts_with(&source) || source.starts_with(&target) {
        return Err("新目录不能包含当前 CARGO_HOME，也不能位于当前 CARGO_HOME 内。".into());
    }
    if target.exists() {
        if !target.is_dir() {
            return Err("目标位置不是目录。".into());
        }
        let mut entries =
            fs::read_dir(&target).map_err(|error| format!("无法读取目标目录：{error}"))?;
        if entries.next().is_some() {
            return Err("目标目录必须为空，请选择空文件夹。".into());
        }
    }
    if create_link && is_link(&default) {
        return Err("默认路径 ~/.cargo 已存在链接，请先确认当前链接状态。".into());
    }
    if create_link && default.exists() && !same_path(&default, &source) {
        return Err("默认路径 ~/.cargo 已存在，无法创建链接。请先选择不创建链接。".into());
    }

    let mut files = 0u64;
    let mut bytes = 0u64;
    if source.exists() {
        (files, bytes) = copy_tree(&source, &target)?;
    } else {
        fs::create_dir_all(&target).map_err(|error| format!("无法创建新的 CARGO_HOME：{error}"))?;
    }

    let backup = if source.exists() {
        let backup = backup_path(&source);
        fs::rename(&source, &backup).map_err(|error| format!("无法移走旧 CARGO_HOME：{error}"))?;
        Some(backup)
    } else {
        None
    };

    let mut link_created = false;
    if create_link {
        if let Err(error) = create_directory_link(&default, &target) {
            restore_source(&backup, &source);
            return Err(error);
        }
        link_created = true;
    }

    let env_output = match set_cargo_home_environment(&target) {
        Ok(output) => output,
        Err(error) => {
            if link_created {
                let _ = remove_directory_link(&default);
            }
            restore_source(&backup, &source);
            return Err(error);
        }
    };

    env::set_var("CARGO_HOME", &target);
    prepend_path(&target.join("bin"));
    let saved_output = match save_cargo_home(&target) {
        Ok(()) => String::new(),
        Err(error) => format!("；保存应用内 CARGO_HOME 失败：{error}"),
    };

    let mut cleanup_warning = String::new();
    if let Some(backup) = backup {
        if let Err(error) = fs::remove_dir_all(&backup) {
            cleanup_warning = format!("；旧目录备份未能删除：{error}");
        }
    }

    let status = status()?;
    let link_text = if create_link {
        format!("已创建链接 {} -> {}", display(&default), display(&target))
    } else {
        "未创建旧路径链接".to_string()
    };
    Ok(CargoHomeMigrationResult {
        output: format!(
            "已迁移 {files} 个文件（{}），CARGO_HOME = {}；{link_text}；{env_output}{saved_output}{cleanup_warning}",
            format_bytes(bytes),
            display(&target)
        ),
        status,
    })
}

pub fn apply_saved_cargo_home() -> Option<PathBuf> {
    if env::var_os("CARGO_HOME").is_some() {
        return None;
    }
    let saved = saved_cargo_home()?;
    env::set_var("CARGO_HOME", &saved);
    prepend_path(&saved.join("bin"));
    Some(saved)
}

fn saved_cargo_home_path() -> Option<PathBuf> {
    dirs::data_local_dir()
        .or_else(dirs::config_dir)
        .map(|directory| directory.join("CargoSweep").join("cargo-home.txt"))
}

fn saved_cargo_home() -> Option<PathBuf> {
    let path = saved_cargo_home_path()?;
    let value = fs::read_to_string(&path).ok()?;
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    let candidate = PathBuf::from(value);
    candidate.is_dir().then_some(candidate)
}

fn save_cargo_home(path: &Path) -> Result<(), String> {
    let Some(config_path) = saved_cargo_home_path() else {
        return Err("无法定位应用配置目录。".into());
    };
    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("无法创建应用配置目录：{error}"))?;
    }
    fs::write(&config_path, display(path))
        .map_err(|error| format!("无法写入应用配置 {}：{error}", config_path.display()))
}

fn default_cargo_home() -> Result<PathBuf, String> {
    dirs::home_dir()
        .map(|home| home.join(".cargo"))
        .ok_or_else(|| "无法定位用户目录。".to_string())
}

fn rustup_home() -> Result<PathBuf, String> {
    if let Some(path) = env::var_os("RUSTUP_HOME") {
        return Ok(PathBuf::from(path));
    }
    dirs::home_dir()
        .map(|home| home.join(".rustup"))
        .ok_or_else(|| "无法定位用户目录。".to_string())
}

fn link_state(default: &Path) -> (bool, Option<PathBuf>) {
    let Ok(metadata) = fs::symlink_metadata(default) else {
        return (false, None);
    };
    if !is_link_metadata(&metadata) {
        return (false, None);
    }
    let target = fs::read_link(default)
        .ok()
        .or_else(|| dunce::canonicalize(default).ok());
    (true, target)
}

fn is_link(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .map(|metadata| is_link_metadata(&metadata))
        .unwrap_or(false)
}

fn is_link_metadata(metadata: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}

fn copy_tree(source: &Path, target: &Path) -> Result<(u64, u64), String> {
    fs::create_dir_all(target).map_err(|error| format!("无法创建新的 CARGO_HOME：{error}"))?;
    let mut files = 0u64;
    let mut bytes = 0u64;
    for entry in WalkDir::new(source).follow_links(false) {
        let entry = entry.map_err(|error| format!("读取 CARGO_HOME 失败：{error}"))?;
        let relative = entry
            .path()
            .strip_prefix(source)
            .map_err(|error| format!("无法计算迁移路径：{error}"))?;
        if relative.as_os_str().is_empty() {
            continue;
        }
        let destination = target.join(relative);
        let metadata = fs::symlink_metadata(entry.path())
            .map_err(|error| format!("读取 {} 失败：{error}", entry.path().display()))?;
        if metadata.file_type().is_symlink() {
            copy_symlink(entry.path(), &destination)?;
            files += 1;
        } else if metadata.is_dir() {
            fs::create_dir_all(&destination)
                .map_err(|error| format!("无法创建 {}：{error}", destination.display()))?;
        } else {
            if let Some(parent) = destination.parent() {
                fs::create_dir_all(parent)
                    .map_err(|error| format!("无法创建 {}：{error}", parent.display()))?;
            }
            fs::copy(entry.path(), &destination)
                .map_err(|error| format!("复制 {} 失败：{error}", entry.path().display()))?;
            files += 1;
            bytes += metadata.len();
        }
    }
    Ok((files, bytes))
}

fn copy_symlink(source: &Path, target: &Path) -> Result<(), String> {
    let link_target = dunce::canonicalize(source)
        .or_else(|_| fs::read_link(source))
        .map_err(|error| format!("读取符号链接 {} 失败：{error}", source.display()))?;
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("无法创建 {}：{error}", parent.display()))?;
    }
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&link_target, target)
            .map_err(|error| format!("复制符号链接到 {} 失败：{error}", target.display()))
    }
    #[cfg(windows)]
    {
        let is_dir = fs::metadata(source)
            .map(|metadata| metadata.is_dir())
            .unwrap_or(false);
        let result = if is_dir {
            std::os::windows::fs::symlink_dir(&link_target, target)
        } else {
            std::os::windows::fs::symlink_file(&link_target, target)
        };
        result.map_err(|error| format!("复制链接到 {} 失败：{error}", target.display()))
    }
}

fn set_cargo_home_environment(target: &Path) -> Result<String, String> {
    #[cfg(windows)]
    {
        let mut command = Command::new("setx");
        command.arg("CARGO_HOME").arg(target);
        apply_no_window(&mut command);
        let output = command
            .output()
            .map_err(|error| format!("无法设置 CARGO_HOME：{error}"))?;
        if !output.status.success() {
            return Err(format!(
                "设置用户环境变量 CARGO_HOME 失败：{}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        Ok("已写入用户环境变量 CARGO_HOME".into())
    }
    #[cfg(unix)]
    {
        let profile = shell_profile().ok_or_else(|| "无法定位 shell 配置文件。".to_string())?;
        write_profile(&profile, target)?;
        Ok(format!("已写入 {}", display(&profile)))
    }
}

#[cfg(unix)]
fn write_profile(profile: &Path, target: &Path) -> Result<(), String> {
    let begin = "# >>> Cargo Sweep CARGO_HOME >>>";
    let end = "# <<< Cargo Sweep CARGO_HOME <<<";
    let mut content = fs::read_to_string(profile).unwrap_or_default();
    if let Some(start) = content.find(begin) {
        if let Some(relative_end) = content[start..].find(end) {
            let finish = start + relative_end + end.len();
            content.replace_range(start..finish, "");
        }
    }
    let escaped = display(target).replace('\'', "'\\''");
    let block = format!("{begin}\nexport CARGO_HOME='{escaped}'\n{end}\n");
    if !content.trim().is_empty() {
        content = content.trim_end().to_string();
        content.push_str("\n\n");
    }
    content.push_str(&block);
    if let Some(parent) = profile.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("无法创建 shell 配置目录：{error}"))?;
    }
    fs::write(profile, content)
        .map_err(|error| format!("写入 shell 配置 {} 失败：{error}", profile.display()))
}

fn create_directory_link(link: &Path, target: &Path) -> Result<(), String> {
    if link.exists() {
        return Err(format!("链接路径 {} 已存在。", display(link)));
    }
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, link).map_err(|error| {
            format!(
                "创建链接 {} -> {} 失败：{error}",
                display(link),
                display(target)
            )
        })
    }
    #[cfg(windows)]
    {
        let mut command = Command::new("cmd");
        command.args(["/C", "mklink", "/J"]).arg(link).arg(target);
        apply_no_window(&mut command);
        let output = command
            .output()
            .map_err(|error| format!("无法创建目录链接：{error}"))?;
        if output.status.success() {
            Ok(())
        } else {
            Err(format!(
                "创建目录链接失败：{}",
                String::from_utf8_lossy(&output.stderr).trim()
            ))
        }
    }
}

fn remove_directory_link(link: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        fs::remove_file(link).map_err(|error| format!("移除链接失败：{error}"))
    }
    #[cfg(windows)]
    {
        fs::remove_dir(link)
            .or_else(|_| fs::remove_file(link))
            .map_err(|error| format!("移除链接失败：{error}"))
    }
}

fn restore_source(backup: &Option<PathBuf>, source: &Path) {
    if let Some(backup) = backup {
        if backup.exists() && !source.exists() {
            let _ = fs::rename(backup, source);
        }
    }
}

fn backup_path(source: &Path) -> PathBuf {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default();
    let parent = source.parent().unwrap_or_else(|| Path::new("."));
    let name = source
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("cargo-home");
    for suffix in 0..1000 {
        let suffix = if suffix == 0 {
            String::new()
        } else {
            format!("-{suffix}")
        };
        let candidate = parent.join(format!(".{name}.cargo-sweep-backup-{timestamp}{suffix}"));
        if !candidate.exists() {
            return candidate;
        }
    }
    parent.join(format!(".{name}.cargo-sweep-backup-{timestamp}-overflow"))
}

fn prepend_path(directory: &Path) {
    let mut paths: Vec<PathBuf> = env::var_os("PATH")
        .map(|value| env::split_paths(&value).collect())
        .unwrap_or_default();
    if paths.iter().any(|path| path == directory) {
        return;
    }
    paths.insert(0, directory.to_path_buf());
    if let Ok(joined) = env::join_paths(paths) {
        env::set_var("PATH", joined);
    }
}

fn path_contains(directory: &Path) -> bool {
    env::var_os("PATH")
        .map(|value| env::split_paths(&value).any(|path| same_path(&path, directory)))
        .unwrap_or(false)
}

fn same_path(left: &Path, right: &Path) -> bool {
    canonical(left)
        .zip(canonical(right))
        .map(|(left, right)| left == right)
        .unwrap_or_else(|| left == right)
}

fn canonical(path: &Path) -> Option<PathBuf> {
    dunce::canonicalize(path).ok()
}

fn canonical_or_self(path: &Path) -> PathBuf {
    canonical(path).unwrap_or_else(|| path.to_path_buf())
}

fn expand_tilde(value: &str) -> PathBuf {
    let value = value.trim().trim_matches('"').trim_matches('\'');
    if value == "~" {
        if let Some(home) = dirs::home_dir() {
            return home;
        }
    }
    if let Some(rest) = value
        .strip_prefix("~/")
        .or_else(|| value.strip_prefix("~\\"))
    {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    }
    PathBuf::from(value)
}

fn display(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KB", "MB", "GB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

#[cfg(unix)]
fn shell_profile() -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    #[cfg(target_os = "macos")]
    {
        Some(home.join(".zprofile"))
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        Some(home.join(".profile"))
    }
}

#[cfg(not(unix))]
fn shell_profile() -> Option<PathBuf> {
    None
}

#[cfg(windows)]
fn apply_no_window(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    command.creation_flags(0x08000000);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_sizes() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(1024), "1.0 KB");
        assert_eq!(format_bytes(1024 * 1024), "1.0 MB");
    }

    #[test]
    fn expands_home_paths() {
        let home = dirs::home_dir().expect("home directory");
        assert_eq!(expand_tilde("~/cargo-home"), home.join("cargo-home"));
    }

    #[test]
    fn detects_cargo_home_environment() {
        let status = status().expect("cargo home status");
        assert!(!status.current.is_empty());
        assert!(!status.cargo_bin.is_empty());
    }

    #[test]
    fn copies_a_cargo_home_tree() {
        let temp = tempfile::tempdir().expect("temporary directory");
        let source = temp.path().join("source");
        let target = temp.path().join("target");
        fs::create_dir_all(source.join("registry").join("cache")).expect("source directories");
        fs::write(source.join("config.toml"), "test = true\n").expect("config");
        fs::write(source.join("registry").join("cache").join("index"), "data").expect("cache file");

        let (files, bytes) = copy_tree(&source, &target).expect("copy tree");
        assert_eq!(files, 2);
        assert!(bytes > 0);
        assert_eq!(
            fs::read_to_string(target.join("config.toml")).expect("copied config"),
            "test = true\n"
        );
        assert!(target
            .join("registry")
            .join("cache")
            .join("index")
            .is_file());
    }

    #[test]
    fn creates_and_detects_a_directory_link() {
        let temp = tempfile::tempdir().expect("temporary directory");
        let target = temp.path().join("cargo-home");
        let link = temp.path().join("old-cargo-home");
        fs::create_dir_all(&target).expect("target");

        create_directory_link(&link, &target).expect("create directory link");
        assert!(is_link(&link));
        let (exists, link_target) = link_state(&link);
        assert!(exists);
        assert!(link_target.is_some());
        remove_directory_link(&link).expect("remove directory link");
        assert!(!link.exists());
    }
}
