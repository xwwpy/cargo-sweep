use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
#[cfg(unix)]
use std::path::Path;
#[cfg(windows)]
use std::process::Command;

const DIST_VAR: &str = "RUSTUP_DIST_SERVER";
const UPDATE_VAR: &str = "RUSTUP_UPDATE_ROOT";
const DEFAULT_DIST: &str = "https://static.rust-lang.org";
const DEFAULT_UPDATE: &str = "https://static.rust-lang.org/rustup";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RustupMirror {
    pub id: String,
    pub name: String,
    pub dist_server: String,
    pub update_root: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RustupSourceStatus {
    pub current_id: String,
    pub current_name: String,
    pub dist_server: String,
    pub update_root: String,
    pub env_configured: bool,
    pub backup_path: Option<String>,
    pub mirrors: Vec<RustupMirror>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct SavedSource {
    dist_server: String,
    update_root: String,
}

pub fn status() -> Result<RustupSourceStatus, String> {
    apply_saved_source();
    let dist_env = env::var(DIST_VAR)
        .ok()
        .filter(|value| !value.trim().is_empty());
    let update_env = env::var(UPDATE_VAR)
        .ok()
        .filter(|value| !value.trim().is_empty());
    let dist = dist_env.clone().unwrap_or_else(|| DEFAULT_DIST.to_string());
    let update = update_env
        .clone()
        .unwrap_or_else(|| DEFAULT_UPDATE.to_string());
    let mirrors = mirrors();
    let matched = mirrors.iter().find(|mirror| {
        same_url(&mirror.dist_server, &dist) && same_url(&mirror.update_root, &update)
    });
    let current_id = matched
        .map(|mirror| mirror.id.clone())
        .unwrap_or_else(|| "custom".into());
    let current_name = matched
        .map(|mirror| mirror.name.clone())
        .unwrap_or_else(|| "自定义".into());

    Ok(RustupSourceStatus {
        current_id,
        current_name,
        dist_server: normalize_url(&dist),
        update_root: normalize_url(&update),
        env_configured: dist_env.is_some() || update_env.is_some(),
        backup_path: source_backup_path()
            .filter(|path| path.is_file())
            .map(|path| path.display().to_string()),
        mirrors,
    })
}

pub fn apply(
    source_id: String,
    dist_server: Option<String>,
    update_root: Option<String>,
) -> Result<RustupSourceStatus, String> {
    let source_id = source_id.trim();
    if source_id == "official" {
        backup_current_source()?;
        clear_saved_source()?;
        clear_profile_var(DIST_VAR)?;
        clear_profile_var(UPDATE_VAR)?;
        env::remove_var(DIST_VAR);
        env::remove_var(UPDATE_VAR);
        return status();
    }

    let mirror = mirrors().into_iter().find(|mirror| mirror.id == source_id);
    let (dist, update) = if let Some(mirror) = mirror {
        (mirror.dist_server, mirror.update_root)
    } else if source_id == "custom" {
        let dist = normalize_url(
            dist_server
                .as_deref()
                .ok_or_else(|| "请输入 RUSTUP_DIST_SERVER。".to_string())?,
        );
        let update = normalize_url(
            update_root
                .as_deref()
                .ok_or_else(|| "请输入 RUSTUP_UPDATE_ROOT。".to_string())?,
        );
        validate_url(&dist)?;
        validate_url(&update)?;
        (dist, update)
    } else {
        return Err("未知的 Rustup 下载源。".into());
    };

    validate_url(&dist)?;
    validate_url(&update)?;
    backup_current_source()?;
    persist_user_env(DIST_VAR, &dist)?;
    persist_user_env(UPDATE_VAR, &update)?;
    env::set_var(DIST_VAR, &dist);
    env::set_var(UPDATE_VAR, &update);
    save_source(&SavedSource {
        dist_server: dist,
        update_root: update,
    })?;
    status()
}

pub fn apply_saved_source() {
    if env::var_os(DIST_VAR).is_some() || env::var_os(UPDATE_VAR).is_some() {
        return;
    }
    let Some(saved) = read_saved_source() else {
        return;
    };
    if validate_url(&saved.dist_server).is_ok() && validate_url(&saved.update_root).is_ok() {
        env::set_var(DIST_VAR, saved.dist_server);
        env::set_var(UPDATE_VAR, saved.update_root);
    }
}

fn mirrors() -> Vec<RustupMirror> {
    vec![
        RustupMirror {
            id: "official".into(),
            name: "官方源".into(),
            dist_server: DEFAULT_DIST.into(),
            update_root: DEFAULT_UPDATE.into(),
        },
        RustupMirror {
            id: "tuna".into(),
            name: "清华 TUNA".into(),
            dist_server: "https://mirrors.tuna.tsinghua.edu.cn/rustup".into(),
            update_root: "https://mirrors.tuna.tsinghua.edu.cn/rustup/rustup".into(),
        },
        RustupMirror {
            id: "ustc".into(),
            name: "中科大 USTC".into(),
            dist_server: "https://mirrors.ustc.edu.cn/rust-static".into(),
            update_root: "https://mirrors.ustc.edu.cn/rust-static/rustup".into(),
        },
        RustupMirror {
            id: "aliyun".into(),
            name: "阿里云".into(),
            dist_server: "https://mirrors.aliyun.com/rustup".into(),
            update_root: "https://mirrors.aliyun.com/rustup/rustup".into(),
        },
        RustupMirror {
            id: "rsproxy".into(),
            name: "rsproxy.cn".into(),
            dist_server: "https://rsproxy.cn".into(),
            update_root: "https://rsproxy.cn/rustup".into(),
        },
    ]
}

fn validate_url(value: &str) -> Result<(), String> {
    if !value.starts_with("https://") {
        return Err("下载源必须使用 HTTPS 地址。".into());
    }
    if value.len() <= "https://".len()
        || value.chars().any(char::is_whitespace)
        || value.contains('"')
        || value.contains('\'')
    {
        return Err("下载源地址格式不正确。".into());
    }
    Ok(())
}

fn normalize_url(value: &str) -> String {
    value.trim().trim_end_matches('/').to_string()
}

fn same_url(left: &str, right: &str) -> bool {
    normalize_url(left).eq_ignore_ascii_case(&normalize_url(right))
}

fn source_path() -> Option<std::path::PathBuf> {
    dirs::data_local_dir()
        .or_else(dirs::config_dir)
        .map(|directory| directory.join("CargoSweep").join("rustup-source.json"))
}

fn source_backup_path() -> Option<std::path::PathBuf> {
    let path = source_path()?;
    let name = path.file_name()?.to_string_lossy();
    Some(path.with_file_name(format!("{name}.old")))
}

fn backup_current_source() -> Result<(), String> {
    let Some(path) = source_path() else {
        return Ok(());
    };
    let backup = source_backup_path().ok_or_else(|| "无法定位备份路径。".to_string())?;
    if let Some(parent) = backup.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("无法创建备份目录：{error}"))?;
    }
    if path.is_file() {
        fs::copy(&path, &backup).map_err(|error| format!("备份 Rustup 下载源配置失败：{error}"))?;
        return Ok(());
    }
    let current = SavedSource {
        dist_server: env::var(DIST_VAR)
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_DIST.to_string()),
        update_root: env::var(UPDATE_VAR)
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_UPDATE.to_string()),
    };
    let content = serde_json::to_string_pretty(&current)
        .map_err(|error| format!("无法序列化 Rustup 下载源备份：{error}"))?;
    fs::write(&backup, content).map_err(|error| format!("无法写入 Rustup 下载源备份：{error}"))
}

fn read_saved_source() -> Option<SavedSource> {
    let path = source_path()?;
    fs::read_to_string(path)
        .ok()
        .and_then(|content| serde_json::from_str(&content).ok())
}

fn save_source(source: &SavedSource) -> Result<(), String> {
    let Some(path) = source_path() else {
        return Err("无法定位应用配置目录。".into());
    };
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("无法创建应用配置目录：{error}"))?;
    }
    let content = serde_json::to_string_pretty(source)
        .map_err(|error| format!("无法序列化 Rustup 下载源配置：{error}"))?;
    fs::write(&path, content).map_err(|error| format!("无法保存 Rustup 下载源配置：{error}"))
}

fn clear_saved_source() -> Result<(), String> {
    let Some(path) = source_path() else {
        return Ok(());
    };
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("无法清除 Rustup 下载源配置：{error}")),
    }
}

#[cfg(windows)]
fn persist_user_env(name: &str, value: &str) -> Result<(), String> {
    let mut command = Command::new("setx");
    command.arg(name).arg(value);
    apply_no_window(&mut command);
    let output = command
        .output()
        .map_err(|error| format!("无法设置 {name}：{error}"))?;
    if !output.status.success() {
        return Err(format!(
            "设置用户环境变量 {name} 失败：{}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(())
}

#[cfg(windows)]
fn clear_profile_var(name: &str) -> Result<(), String> {
    let mut command = Command::new("reg");
    command.args(["delete", "HKCU\\Environment", "/v", name, "/f"]);
    apply_no_window(&mut command);
    let _ = command.output();
    Ok(())
}

#[cfg(unix)]
fn persist_user_env(name: &str, value: &str) -> Result<(), String> {
    let profile = shell_profile().ok_or_else(|| "无法定位 shell 配置文件。".to_string())?;
    write_profile(name, &profile, Some(value))
}

#[cfg(unix)]
fn clear_profile_var(name: &str) -> Result<(), String> {
    if let Some(profile) = shell_profile() {
        write_profile(name, &profile, None)?;
    }
    Ok(())
}

#[cfg(unix)]
fn write_profile(name: &str, profile: &Path, value: Option<&str>) -> Result<(), String> {
    let begin = format!("# >>> Cargo Sweep {name} >>>");
    let end = format!("# <<< Cargo Sweep {name} <<<");
    let mut content = fs::read_to_string(profile).unwrap_or_default();
    if let Some(start) = content.find(&begin) {
        if let Some(relative_end) = content[start..].find(&end) {
            let finish = start + relative_end + end.len();
            content.replace_range(start..finish, "");
        }
    }
    if let Some(value) = value {
        let escaped = value.replace('\'', "'\\''");
        let block = format!("{begin}\nexport {name}='{escaped}'\n{end}\n");
        if !content.trim().is_empty() {
            content = content.trim_end().to_string();
            content.push_str("\n\n");
        }
        content.push_str(&block);
    }
    if let Some(parent) = profile.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("无法创建 shell 配置目录：{error}"))?;
    }
    fs::write(profile, content)
        .map_err(|error| format!("写入 shell 配置 {} 失败：{error}", profile.display()))
}

#[cfg(unix)]
fn shell_profile() -> Option<std::path::PathBuf> {
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

#[cfg(windows)]
fn apply_no_window(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    command.creation_flags(0x08000000);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_and_matches_urls() {
        assert_eq!(normalize_url("https://example.com/"), "https://example.com");
        assert!(same_url("https://example.com/", "https://EXAMPLE.com"));
        assert!(!same_url("https://example.com", "https://other.com"));
    }

    #[test]
    fn validates_https_sources() {
        assert!(validate_url("https://mirrors.example.com/rustup").is_ok());
        assert!(validate_url("http://mirrors.example.com/rustup").is_err());
        assert!(validate_url("not-a-url").is_err());
    }
}
