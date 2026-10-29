use crate::background;
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};
use toml::Value;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledToolchain {
    pub name: String,
    pub channel: String,
    pub release: Option<String>,
    pub rustc_version: Option<String>,
    pub active: bool,
    pub default: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RustChannel {
    pub channel: String,
    pub installed: bool,
    pub toolchain: Option<String>,
    pub release: Option<String>,
    pub rustc_version: Option<String>,
    pub active: bool,
    pub default: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RustToolchainStatus {
    pub installed: bool,
    pub rustup_installed: bool,
    pub version: Option<String>,
    pub rustc_version: Option<String>,
    pub host: Option<String>,
    pub active_toolchain: Option<String>,
    pub default_toolchain: Option<String>,
    pub toolchains: Vec<InstalledToolchain>,
    pub channels: Vec<RustChannel>,
    pub cargo_version: Option<String>,
    pub rustup_version: Option<String>,
    pub rustup_path: Option<String>,
    pub rustup_custom_path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LatestRustToolchain {
    pub channel: String,
    pub version: String,
    pub release: String,
    pub date: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LatestRustVersions {
    pub stable: Option<LatestRustToolchain>,
    pub nightly: Option<LatestRustToolchain>,
    pub historical_versions: Vec<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolchainOperationResult {
    pub success: bool,
    pub output: String,
    pub status: RustToolchainStatus,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComponentProgress {
    pub name: String,
    pub phase: String,
    pub percent: u8,
    pub elapsed_ms: u64,
    pub speed_bps: u64,
    pub indeterminate: bool,
}

#[derive(Debug, Clone)]
pub struct OperationProgress {
    pub percent: u8,
    pub message: String,
    pub phase: String,
    pub line: Option<String>,
    pub components: Vec<ComponentProgress>,
    pub speed_bps: u64,
    pub elapsed_ms: u64,
    pub downloaded_bytes: u64,
}

impl OperationProgress {
    fn new(percent: u8, message: impl Into<String>) -> Self {
        Self {
            percent,
            message: message.into(),
            phase: "verify".into(),
            line: None,
            components: Vec::new(),
            speed_bps: 0,
            elapsed_ms: 0,
            downloaded_bytes: 0,
        }
    }
}

impl Default for OperationProgress {
    fn default() -> Self {
        Self::new(0, "")
    }
}

impl Serialize for OperationProgress {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("OperationProgress", 8)?;
        state.serialize_field("percent", &self.percent)?;
        state.serialize_field("message", &self.message)?;
        state.serialize_field("phase", &self.phase)?;
        state.serialize_field("line", &self.line)?;
        state.serialize_field("components", &self.components)?;
        state.serialize_field("speedBps", &self.speed_bps)?;
        state.serialize_field("elapsedMs", &self.elapsed_ms)?;
        state.serialize_field("downloadedBytes", &self.downloaded_bytes)?;
        state.end()
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RustInstallationProbe {
    pub installed: bool,
    pub rustup_installed: bool,
    pub version: Option<String>,
    pub rustup_path: Option<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct AppSettings {
    #[serde(default)]
    rustup_path: Option<String>,
}

pub fn probe() -> RustInstallationProbe {
    crate::cargo_home::apply_saved_homes();
    crate::rustup_source::apply_saved_source();
    let rustup_path = prepare_rust_environment();
    let rustc_version = run("rustc", &["--version"])
        .ok()
        .and_then(|output| first_prefixed_line(&output, "rustc "));
    let rustup_version = run("rustup", &["--version"])
        .ok()
        .and_then(|output| first_prefixed_line(&output, "rustup "));
    RustInstallationProbe {
        installed: rustc_version.is_some(),
        rustup_installed: rustup_version.is_some(),
        version: rustc_version,
        rustup_path: rustup_path.map(|path| path.to_string_lossy().to_string()),
    }
}

pub fn detect() -> Result<RustToolchainStatus, String> {
    crate::cargo_home::apply_saved_homes();
    crate::rustup_source::apply_saved_source();
    let rustup_path = prepare_rust_environment();
    let settings = read_settings();
    let rustup_version = run("rustup", &["--version"])
        .ok()
        .and_then(|output| first_prefixed_line(&output, "rustup "))
        .map(|version| strip_prefix(&version, "rustup "));
    let rustup_installed = rustup_version.is_some();

    let active_toolchain = if rustup_installed {
        run("rustup", &["show", "active-toolchain"])
            .ok()
            .and_then(|output| parse_active_toolchain(&output))
    } else {
        None
    };
    let default_toolchain = if rustup_installed {
        run("rustup", &["default"])
            .ok()
            .and_then(|output| parse_active_toolchain(&output))
    } else {
        None
    };

    let list_output = if rustup_installed {
        run("rustup", &["toolchain", "list"]).unwrap_or_default()
    } else {
        String::new()
    };
    let mut toolchains = Vec::new();
    for line in list_output.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let name = line.split_whitespace().next().unwrap_or(line).to_string();
        let plus = format!("+{name}");
        let version_output = run("rustc", &[plus.as_str(), "--version", "--verbose"]).ok();
        let (rustc_version, release, _host) = version_output
            .as_deref()
            .map(parse_rustc_verbose)
            .unwrap_or((String::new(), String::new(), None));
        toolchains.push(InstalledToolchain {
            channel: channel_from_toolchain(&name),
            name: name.clone(),
            release: if release.is_empty() {
                None
            } else {
                Some(release)
            },
            rustc_version: if rustc_version.is_empty() {
                None
            } else {
                Some(rustc_version)
            },
            active: line.contains("(active") || active_toolchain.as_deref() == Some(name.as_str()),
            default: line.contains("default")
                || default_toolchain.as_deref() == Some(name.as_str()),
        });
    }

    let default_output = run("rustc", &["--version", "--verbose"]).ok();
    let (default_rustc_version, default_release, default_host) = default_output
        .as_deref()
        .map(parse_rustc_verbose)
        .unwrap_or((String::new(), String::new(), None));

    let active_info = toolchains
        .iter()
        .find(|toolchain| toolchain.active)
        .or_else(|| toolchains.iter().find(|toolchain| toolchain.default));
    let active_release = active_info
        .and_then(|toolchain| toolchain.release.clone())
        .or_else(|| (!default_release.is_empty()).then_some(default_release));
    let active_rustc_version = active_info
        .and_then(|toolchain| toolchain.rustc_version.clone())
        .or_else(|| (!default_rustc_version.is_empty()).then_some(default_rustc_version));
    let active_host = if let Some(toolchain) = active_info {
        let plus = format!("+{}", toolchain.name);
        run("rustc", &[plus.as_str(), "--version", "--verbose"])
            .ok()
            .and_then(|output| parse_rustc_verbose(&output).2)
            .or(default_host)
    } else {
        default_host
    };

    let channels = ["stable", "nightly"]
        .iter()
        .map(|channel| {
            let installed = toolchains
                .iter()
                .find(|toolchain| toolchain.channel == *channel);
            RustChannel {
                channel: (*channel).to_string(),
                installed: installed.is_some(),
                toolchain: installed.map(|toolchain| toolchain.name.clone()),
                release: installed.and_then(|toolchain| toolchain.release.clone()),
                rustc_version: installed.and_then(|toolchain| toolchain.rustc_version.clone()),
                active: installed.is_some_and(|toolchain| toolchain.active),
                default: installed.is_some_and(|toolchain| toolchain.default),
            }
        })
        .collect();

    let cargo_version = run("cargo", &["--version"])
        .ok()
        .and_then(|output| first_prefixed_line(&output, "cargo "))
        .map(|version| strip_prefix(&version, "cargo "));

    Ok(RustToolchainStatus {
        installed: active_rustc_version.is_some(),
        rustup_installed,
        version: active_release,
        rustc_version: active_rustc_version,
        host: active_host,
        active_toolchain,
        default_toolchain,
        toolchains,
        channels,
        cargo_version,
        rustup_version,
        rustup_path: rustup_path.map(|path| path.to_string_lossy().to_string()),
        rustup_custom_path: settings.rustup_path,
    })
}

pub fn latest() -> Result<LatestRustVersions, String> {
    let stable = fetch_channel("stable");
    let nightly = fetch_channel("nightly");
    let mut errors = Vec::new();
    if let Err(error) = &stable {
        errors.push(error.clone());
    }
    if let Err(error) = &nightly {
        errors.push(error.clone());
    }

    let stable = stable.ok();
    let nightly = nightly.ok();
    let historical_versions = stable
        .as_ref()
        .map(|release| fetch_historical_versions(&release.release))
        .unwrap_or_default();

    if stable.is_none() && nightly.is_none() {
        return Err(if errors.is_empty() {
            "无法获取 Rust 最新版本。".into()
        } else {
            errors.join("；")
        });
    }

    Ok(LatestRustVersions {
        stable,
        nightly,
        historical_versions,
        error: (!errors.is_empty()).then(|| errors.join("；")),
    })
}

pub fn install_with_progress<F>(
    target: String,
    set_default: bool,
    mut on_progress: F,
) -> Result<ToolchainOperationResult, String>
where
    F: FnMut(OperationProgress),
{
    let target = validate_target(&target)?;
    let before = detect()?;
    if !before.rustup_installed {
        return Err("未检测到 rustup，请先安装 Rustup 后再下载工具链。".into());
    }

    on_progress(OperationProgress::new(2, format!("准备下载 {target}")));
    let is_versioned = target
        .chars()
        .next()
        .is_some_and(|character| character.is_ascii_digit());
    let custom_source =
        env::var_os("RUSTUP_DIST_SERVER").is_some() || env::var_os("RUSTUP_UPDATE_ROOT").is_some();
    let monitor = DownloadMonitor::start();
    let mut progress = RustupProgress::new(monitor.meter());
    let (mut success, mut text) = run_streaming(
        "rustup",
        &["toolchain", "install", &target, "--profile", "default"],
        |line| {
            on_progress(progress.observe(line));
        },
    )?;

    if !success && !background::current_cancelled() && is_versioned && custom_source {
        on_progress(OperationProgress::new(
            5,
            "当前下载源缺少该版本，改用官方源重试",
        ));
        let mut fallback_progress = RustupProgress::new(monitor.meter());
        let (fallback_success, fallback_text) = run_streaming_with_env(
            "rustup",
            &["toolchain", "install", &target, "--profile", "default"],
            &["RUSTUP_DIST_SERVER", "RUSTUP_UPDATE_ROOT"],
            |line| {
                on_progress(fallback_progress.observe(line));
            },
        )?;
        success = fallback_success;
        progress = fallback_progress;
        text = if fallback_text.trim().is_empty() {
            format!("{text}\n\n[已改用官方源重试] 未返回输出。")
        } else {
            format!("{text}\n\n[已改用官方源重试]\n{fallback_text}")
        };
    }

    if success && (set_default || before.default_toolchain.is_none()) {
        on_progress(OperationProgress::new(97, "设置默认工具链"));
        let default_output = run("rustup", &["default", &target]);
        match default_output {
            Ok(default_text) => {
                if !default_text.trim().is_empty() {
                    text.push('\n');
                    text.push_str(default_text.trim());
                }
            }
            Err(error) => {
                return Err(format!("工具链已安装，但设置默认工具链失败：{error}"));
            }
        }
    }

    if success {
        on_progress(progress.finish("工具链安装完成"));
    }
    let status = detect().unwrap_or(before);
    Ok(ToolchainOperationResult {
        success,
        output: if text.trim().is_empty() {
            "rustup 已完成操作。".into()
        } else {
            text.trim().to_string()
        },
        status,
    })
}

pub fn set_default(target: String) -> Result<ToolchainOperationResult, String> {
    let target = validate_target(&target)?;
    let before = detect()?;
    if !before.rustup_installed {
        return Err("未检测到 rustup，无法切换工具链。".into());
    }
    let output = run_capture("rustup", &["default", &target])?;
    let status = detect().unwrap_or(before);
    Ok(ToolchainOperationResult {
        success: output.0,
        output: if output.1.trim().is_empty() {
            format!("已切换默认工具链为 {target}")
        } else {
            output.1.trim().to_string()
        },
        status,
    })
}

pub fn update_with_progress<F>(
    target: Option<String>,
    mut on_progress: F,
) -> Result<ToolchainOperationResult, String>
where
    F: FnMut(OperationProgress),
{
    let target = target.map(|value| validate_target(&value)).transpose()?;
    let before = detect()?;
    let toolchain = target
        .or_else(|| before.active_toolchain.clone())
        .or_else(|| before.default_toolchain.clone());

    on_progress(OperationProgress::new(2, "检查工具链更新"));
    let mut args = vec!["update", "--no-self-update"];
    if let Some(toolchain) = toolchain.as_deref() {
        args.push(toolchain);
    }
    let monitor = DownloadMonitor::start();
    let mut progress = RustupProgress::new(monitor.meter());
    let (mut success, mut text) = run_streaming("rustup", &args, |line| {
        on_progress(progress.observe(line));
    })?;
    if !success && text.trim().is_empty() {
        return Err("rustup update 失败，未返回输出。".into());
    }
    if !success && !background::current_cancelled() && needs_toolchain_repair(&text) {
        if let Some(toolchain_name) = toolchain.clone() {
            on_progress(OperationProgress::new(
                4,
                "检测到工具链文件冲突，正在自动修复",
            ));
            let was_default = before.default_toolchain.as_deref() == Some(toolchain_name.as_str());
            match repair_toolchain(&toolchain_name, was_default, &mut on_progress) {
                Ok((repair_success, repair_text)) => {
                    success = repair_success;
                    text = format!("{text}\n\n{repair_text}");
                }
                Err(error) => {
                    text = format!("{text}\n\n[自动修复失败]\n{error}");
                }
            }
        }
    }
    if success {
        on_progress(progress.finish(if text.trim().is_empty() {
            "没有新的更新"
        } else {
            "工具链更新完成"
        }));
    }
    let status = detect().unwrap_or(before);
    Ok(ToolchainOperationResult {
        success,
        output: if text.trim().is_empty() {
            "没有新的更新。".into()
        } else {
            text.trim().to_string()
        },
        status,
    })
}

fn needs_toolchain_repair(output: &str) -> bool {
    let lower = output.to_ascii_lowercase();
    [
        "os error 145",
        "directory not empty",
        "目录不是空的",
        "detected conflict",
        "could not rename component",
        "failed to install component",
    ]
    .iter()
    .any(|needle| lower.contains(needle) || output.contains(needle))
}

fn repair_toolchain<F>(
    toolchain: &str,
    was_default: bool,
    on_progress: &mut F,
) -> Result<(bool, String), String>
where
    F: FnMut(OperationProgress),
{
    let toolchains_dir =
        rustup_toolchains_dir().ok_or_else(|| "无法定位 RUSTUP_HOME/toolchains。".to_string())?;
    let toolchain_dir = toolchains_dir.join(toolchain);
    let components = installed_packages("component", toolchain);
    let targets = installed_packages("target", toolchain);
    let mut output = String::from("[自动修复] 检测到工具链文件冲突，正在重新安装。");

    on_progress(OperationProgress::new(6, "清理损坏的工具链"));
    if let Ok((_, text)) = run_capture("rustup", &["toolchain", "uninstall", toolchain]) {
        append_output(&mut output, &text);
    }

    let backup = if toolchain_dir.exists() {
        let backup = repair_backup_path(&toolchain_dir);
        fs::rename(&toolchain_dir, &backup).map_err(|error| {
            format!(
                "无法移动损坏的工具链目录 {}：{error}",
                toolchain_dir.display()
            )
        })?;
        append_output(
            &mut output,
            &format!("已将损坏目录移动到 {}", backup.display()),
        );
        Some(backup)
    } else {
        None
    };

    let mut progress = RustupProgress::new(Arc::new(Mutex::new(DownloadMeter::default())));
    let install_result = run_streaming(
        "rustup",
        &["toolchain", "install", toolchain, "--profile", "default"],
        |line| {
            on_progress(progress.observe(line));
        },
    );
    let (mut installed, install_text) = match install_result {
        Ok(result) => result,
        Err(error) => {
            restore_repair_backup(&toolchain_dir, backup.as_deref());
            append_output(&mut output, &format!("重新安装失败：{error}"));
            return Ok((false, output));
        }
    };
    append_output(&mut output, &install_text);

    if !installed {
        restore_repair_backup(&toolchain_dir, backup.as_deref());
        append_output(&mut output, "\n工具链重新安装失败，已保留原目录备份。");
        return Ok((false, output));
    }

    on_progress(OperationProgress::new(94, "恢复额外组件和目标"));
    for component in components {
        if let Ok((_, text)) = run_capture(
            "rustup",
            &["component", "add", "--toolchain", toolchain, &component],
        ) {
            append_output(&mut output, &text);
        }
    }
    for target in targets {
        if let Ok((_, text)) = run_capture(
            "rustup",
            &["target", "add", "--toolchain", toolchain, &target],
        ) {
            append_output(&mut output, &text);
        }
    }
    if was_default {
        match run("rustup", &["default", toolchain]) {
            Ok(text) => append_output(&mut output, &text),
            Err(error) => {
                installed = false;
                append_output(&mut output, &format!("恢复默认工具链失败：{error}"));
            }
        }
    }

    if installed {
        if let Some(backup) = backup {
            let _ = fs::remove_dir_all(backup);
        }
        on_progress(progress.finish("工具链修复完成"));
    } else {
        restore_repair_backup(&toolchain_dir, backup.as_deref());
    }
    Ok((installed, output))
}

fn rustup_toolchains_dir() -> Option<PathBuf> {
    env::var_os("RUSTUP_HOME")
        .map(PathBuf::from)
        .or_else(|| dirs::home_dir().map(|home| home.join(".rustup")))
        .map(|home| home.join("toolchains"))
}

fn repair_backup_path(toolchain_dir: &Path) -> PathBuf {
    let name = toolchain_dir
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("toolchain");
    let mut candidate = toolchain_dir.with_file_name(format!("{name}.cargo-sweep-repair"));
    let mut suffix = 1usize;
    while candidate.exists() {
        candidate = toolchain_dir.with_file_name(format!("{name}.cargo-sweep-repair-{suffix}"));
        suffix += 1;
    }
    candidate
}

fn restore_repair_backup(toolchain_dir: &Path, backup: Option<&Path>) {
    let Some(backup) = backup else {
        return;
    };
    if toolchain_dir.exists() {
        let _ = fs::remove_dir_all(toolchain_dir);
    }
    if backup.exists() {
        let _ = fs::rename(backup, toolchain_dir);
    }
}

fn installed_packages(kind: &str, toolchain: &str) -> Vec<String> {
    run(
        "rustup",
        &[kind, "list", "--installed", "--toolchain", toolchain],
    )
    .unwrap_or_default()
    .lines()
    .map(str::trim)
    .filter(|line| !line.is_empty())
    .map(ToOwned::to_owned)
    .collect()
}

fn append_output(output: &mut String, text: &str) {
    if text.trim().is_empty() {
        return;
    }
    if !output.is_empty() {
        output.push('\n');
    }
    output.push_str(text.trim());
}

pub fn uninstall(target: String) -> Result<ToolchainOperationResult, String> {
    let target = validate_target(&target)?;
    let before = detect()?;
    let toolchain = before
        .toolchains
        .iter()
        .find(|toolchain| toolchain.name == target)
        .ok_or_else(|| format!("未找到工具链 {target}。"))?;
    if toolchain.default || toolchain.active {
        return Err("默认工具链不能直接删除，请先切换到其他工具链。".into());
    }
    if before.toolchains.len() <= 1 {
        return Err("至少需要保留一个 Rust 工具链。".into());
    }

    let mut command = Command::new("rustup");
    command.args(["toolchain", "uninstall", &target]);
    apply_no_window(&mut command);
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let output = command
        .output()
        .map_err(|error| format!("无法启动 rustup toolchain uninstall：{error}"))?;
    let success = output.status.success();
    let text = clean_output(&format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    ));
    let status = detect().unwrap_or(before);
    Ok(ToolchainOperationResult {
        success,
        output: if text.trim().is_empty() {
            if success {
                format!("已删除工具链 {target}")
            } else {
                format!("删除工具链 {target} 失败。")
            }
        } else {
            text.trim().to_string()
        },
        status,
    })
}

pub fn set_rustup_path(path: String) -> Result<RustToolchainStatus, String> {
    let trimmed = path.trim().trim_matches('"').trim_matches('\'');
    let custom_path = if trimmed.is_empty() {
        None
    } else {
        let candidate = normalize_rustup_path(&expand_tilde(trimmed));
        if !candidate.is_file() {
            return Err(format!("未找到 rustup 可执行文件：{}", candidate.display()));
        }
        Some(
            dunce::canonicalize(&candidate)
                .unwrap_or(candidate)
                .to_string_lossy()
                .to_string(),
        )
    };

    let mut settings = read_settings();
    settings.rustup_path = custom_path;
    write_settings(&settings)?;
    prepare_rust_environment();
    detect()
}

pub fn install_rustup_with_progress<F>(
    mut on_progress: F,
) -> Result<ToolchainOperationResult, String>
where
    F: FnMut(OperationProgress),
{
    if background::current_cancelled() {
        return Err("任务已取消。".into());
    }
    crate::rustup_source::apply_saved_source();
    prepare_rust_environment();
    if run("rustup", &["--version"]).is_ok() {
        return Err("已检测到 rustup，无需重复安装。".into());
    }

    #[cfg(windows)]
    if let Some(winget_result) = install_rustup_with_winget() {
        let mut text =
            winget_result.map_err(|error| format!("winget 安装 rustup 失败：{error}"))?;
        prepare_rust_environment();
        if run("rustup", &["--version"]).is_ok() {
            on_progress(OperationProgress::new(72, "安装 Stable 工具链"));
            ensure_stable_toolchain(&mut text)?;
            on_progress(OperationProgress::new(100, "rustup 安装完成"));
            let status = detect()?;
            return Ok(ToolchainOperationResult {
                success: true,
                output: text,
                status,
            });
        }
        return Err("winget 已结束，但未检测到 rustup，请检查 winget 输出。".into());
    }

    if background::current_cancelled() {
        return Err("任务已取消。".into());
    }
    on_progress(OperationProgress::new(3, "准备下载 rustup-init"));
    let target = rustup_target()?;
    let executable_name = if cfg!(windows) {
        "rustup-init.exe"
    } else {
        "rustup-init"
    };
    let url = format!("https://static.rust-lang.org/rustup/dist/{target}/{executable_name}");
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(180))
        .user_agent(format!("cargo-sweep/{}", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|error| format!("无法创建网络请求：{error}"))?;
    let mut response = client
        .get(&url)
        .send()
        .map_err(|error| format!("无法下载 rustup-init：{error}"))?;
    if !response.status().is_success() {
        return Err(format!("无法下载 rustup-init：HTTP {}", response.status()));
    }
    let total = response.content_length();
    let mut bytes = Vec::with_capacity(total.unwrap_or(0) as usize);
    let mut buffer = [0u8; 64 * 1024];
    let mut last_percent = 3u8;
    loop {
        let read = response
            .read(&mut buffer)
            .map_err(|error| format!("无法读取 rustup-init 下载内容：{error}"))?;
        if read == 0 {
            break;
        }
        bytes.extend_from_slice(&buffer[..read]);
        if let Some(total) = total.filter(|total| *total > 0) {
            let percent = (5 + bytes.len() as u64 * 70 / total).min(75) as u8;
            if percent >= last_percent + 2 {
                last_percent = percent;
                on_progress(OperationProgress::new(
                    percent,
                    format!("下载 rustup-init · {percent}%"),
                ));
            }
        }
    }
    if bytes.len() < 1024 * 1024 {
        return Err("下载的 rustup-init 文件不完整，请重试。".into());
    }
    on_progress(OperationProgress::new(78, "校验 rustup-init"));
    verify_download(&client, &url, &bytes)?;

    let install_dir = dirs::data_local_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("CargoSweep")
        .join("install");
    fs::create_dir_all(&install_dir).map_err(|error| format!("无法创建安装缓存目录：{error}"))?;
    let binary = install_dir.join(executable_name);
    fs::write(&binary, &bytes).map_err(|error| format!("无法写入 rustup-init：{error}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&binary, fs::Permissions::from_mode(0o755))
            .map_err(|error| format!("无法设置 rustup-init 执行权限：{error}"))?;
    }
    on_progress(OperationProgress::new(85, "安装 Stable 工具链"));

    let before = detect()?;
    let mut command = Command::new(&binary);
    command.args([
        "-y",
        "--default-toolchain",
        "stable",
        "--profile",
        "default",
    ]);
    apply_no_window(&mut command);
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let output = command
        .output()
        .map_err(|error| format!("无法启动 rustup-init：{error}"))?;
    let success = output.status.success();
    let text = clean_output(&format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    ));
    let _ = fs::remove_file(&binary);
    if success {
        on_progress(OperationProgress::new(100, "rustup 安装完成"));
    }

    prepare_rust_environment();
    let status = detect().unwrap_or(before);
    Ok(ToolchainOperationResult {
        success,
        output: if text.trim().is_empty() {
            if success {
                "rustup 已安装。".into()
            } else {
                "rustup-init 未返回输出。".into()
            }
        } else {
            text.trim().to_string()
        },
        status,
    })
}

fn verify_download(
    client: &reqwest::blocking::Client,
    url: &str,
    bytes: &[u8],
) -> Result<(), String> {
    use sha2::{Digest, Sha256};

    let checksum_response = client
        .get(format!("{url}.sha256"))
        .send()
        .map_err(|error| format!("无法获取 rustup-init 校验值：{error}"))?;
    if !checksum_response.status().is_success() {
        return Err(format!(
            "无法获取 rustup-init 校验值：HTTP {}",
            checksum_response.status()
        ));
    }
    let checksum_text = checksum_response
        .text()
        .map_err(|error| format!("无法读取 rustup-init 校验值：{error}"))?;
    let expected = checksum_text
        .split_whitespace()
        .next()
        .ok_or_else(|| "rustup-init 校验值为空。".to_string())?;
    if expected.len() != 64
        || !expected
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    {
        return Err("rustup-init 校验值格式不正确。".into());
    }
    let actual = format!("{:x}", Sha256::digest(bytes));
    if !actual.eq_ignore_ascii_case(expected) {
        return Err("rustup-init 校验失败，已停止安装。".into());
    }
    Ok(())
}

#[cfg(windows)]
fn install_rustup_with_winget() -> Option<Result<String, String>> {
    let mut command = Command::new("winget");
    command.args([
        "install",
        "--id",
        "Rustlang.Rustup",
        "-e",
        "--silent",
        "--accept-package-agreements",
        "--accept-source-agreements",
    ]);
    apply_no_window(&mut command);
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let output = match command.output() {
        Ok(output) => output,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return None,
        Err(error) => return Some(Err(format!("无法启动 winget：{error}"))),
    };
    let text = clean_output(&format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    ));
    if output.status.success() {
        Some(Ok(if text.trim().is_empty() {
            "已通过 winget 安装 rustup。".into()
        } else {
            text
        }))
    } else {
        Some(Err(if text.trim().is_empty() {
            "winget 安装 rustup 失败。".into()
        } else {
            text
        }))
    }
}

fn ensure_stable_toolchain(output: &mut String) -> Result<(), String> {
    prepare_rust_environment();
    let list = run("rustup", &["toolchain", "list"]).unwrap_or_default();
    if !list
        .lines()
        .any(|line| line.trim_start().starts_with("stable-"))
    {
        let result = run_capture(
            "rustup",
            &["toolchain", "install", "stable", "--profile", "default"],
        )?;
        if !result.1.trim().is_empty() {
            output.push('\n');
            output.push_str(result.1.trim());
        }
        if !result.0 {
            return Err("Stable 工具链安装失败。".into());
        }
    }
    if run("rustup", &["default"])
        .unwrap_or_default()
        .trim()
        .is_empty()
    {
        let result = run_capture("rustup", &["default", "stable"])?;
        if !result.0 {
            return Err("设置 Stable 为默认工具链失败。".into());
        }
    }
    Ok(())
}

fn prepare_rust_environment() -> Option<PathBuf> {
    if let Some(bin) = cargo_bin_dir() {
        if bin.is_dir() {
            prepend_path(&bin);
        }
    }

    let rustup = resolve_rustup()?;
    if let Some(parent) = rustup.parent() {
        prepend_path(parent);
    }
    Some(rustup)
}

fn resolve_rustup() -> Option<PathBuf> {
    let mut candidates = Vec::new();

    if let Some(custom) = read_settings().rustup_path {
        candidates.push(normalize_rustup_path(Path::new(&custom)));
    }
    if let Some(bin) = cargo_bin_dir() {
        candidates.push(bin.join(binary_name("rustup")));
    }
    if let Some(path) = find_on_path("rustup") {
        candidates.push(path);
    }

    #[cfg(unix)]
    {
        for directory in [
            "/opt/homebrew/bin",
            "/usr/local/bin",
            "/usr/bin",
            "/opt/local/bin",
        ] {
            candidates.push(Path::new(directory).join(binary_name("rustup")));
        }
    }

    candidates
        .into_iter()
        .find(|candidate| candidate.is_file())
        .map(|candidate| dunce::canonicalize(&candidate).unwrap_or(candidate))
}

fn find_on_path(program: &str) -> Option<PathBuf> {
    let executable = binary_name(program);
    env::var_os("PATH")
        .into_iter()
        .flat_map(|paths| env::split_paths(&paths).collect::<Vec<_>>())
        .map(|directory| directory.join(&executable))
        .find(|candidate| candidate.is_file())
}

fn cargo_bin_dir() -> Option<PathBuf> {
    cargo_home().map(|home| home.join("bin"))
}

fn cargo_home() -> Option<PathBuf> {
    env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .or_else(|| dirs::home_dir().map(|home| home.join(".cargo")))
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

fn binary_name(program: &str) -> String {
    if cfg!(windows) {
        format!("{program}.exe")
    } else {
        program.to_string()
    }
}

fn expand_tilde(value: &str) -> PathBuf {
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

fn normalize_rustup_path(path: &Path) -> PathBuf {
    if path.is_dir() {
        path.join(binary_name("rustup"))
    } else {
        path.to_path_buf()
    }
}

fn rustup_target() -> Result<&'static str, String> {
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    {
        return Ok("x86_64-pc-windows-msvc");
    }
    #[cfg(all(target_os = "windows", target_arch = "aarch64"))]
    {
        return Ok("aarch64-pc-windows-msvc");
    }
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    {
        return Ok("aarch64-apple-darwin");
    }
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    {
        return Ok("x86_64-apple-darwin");
    }
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        return Ok("x86_64-unknown-linux-gnu");
    }
    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    {
        return Ok("aarch64-unknown-linux-gnu");
    }
    #[allow(unreachable_code)]
    Err("当前系统暂不支持一键安装 rustup，请手动安装。".into())
}

fn settings_path() -> Option<PathBuf> {
    dirs::data_local_dir()
        .or_else(dirs::config_dir)
        .map(|directory| directory.join("CargoSweep").join("settings.json"))
}

fn read_settings() -> AppSettings {
    let Some(path) = settings_path() else {
        return AppSettings::default();
    };
    fs::read_to_string(path)
        .ok()
        .and_then(|content| serde_json::from_str(&content).ok())
        .unwrap_or_default()
}

fn write_settings(settings: &AppSettings) -> Result<(), String> {
    let Some(path) = settings_path() else {
        return Err("无法定位应用配置目录。".into());
    };
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("无法创建应用配置目录：{error}"))?;
    }
    let content = serde_json::to_string_pretty(settings)
        .map_err(|error| format!("无法序列化应用配置：{error}"))?;
    fs::write(&path, content).map_err(|error| format!("无法保存应用配置：{error}"))
}

fn fetch_channel(channel: &str) -> Result<LatestRustToolchain, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(8))
        .user_agent(format!("cargo-sweep/{}", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|error| format!("无法创建网络请求：{error}"))?;
    let url = format!("https://static.rust-lang.org/dist/channel-rust-{channel}.toml");
    let response = client
        .get(url)
        .send()
        .map_err(|error| format!("无法获取 {channel} 最新版本：{error}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "无法获取 {channel} 最新版本：HTTP {}",
            response.status()
        ));
    }
    let content = response
        .text()
        .map_err(|error| format!("无法读取 {channel} 版本信息：{error}"))?;
    let document: Value = toml::from_str(&content)
        .map_err(|error| format!("无法解析 {channel} 版本信息：{error}"))?;
    let version = document
        .get("pkg")
        .and_then(Value::as_table)
        .and_then(|pkg| pkg.get("rust"))
        .and_then(Value::as_table)
        .and_then(|rust| rust.get("version"))
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{channel} 版本信息缺少 pkg.rust.version"))?;
    let (release, date) = split_manifest_version(version);
    Ok(LatestRustToolchain {
        channel: channel.to_string(),
        version: version.to_string(),
        release,
        date,
    })
}

fn split_manifest_version(version: &str) -> (String, Option<String>) {
    let release = version
        .split_whitespace()
        .next()
        .unwrap_or(version)
        .to_string();
    let date = version
        .contains('(')
        .then(|| {
            version
                .split_whitespace()
                .last()
                .map(|value| value.trim_end_matches(')').to_string())
        })
        .flatten()
        .filter(|value| value.chars().next().is_some_and(|c| c.is_ascii_digit()));
    (release, date)
}

type HistoryCache = Option<(String, Instant, Vec<String>)>;

static HISTORY_CACHE: OnceLock<Mutex<HistoryCache>> = OnceLock::new();

fn fetch_historical_versions(current_release: &str) -> Vec<String> {
    let cache = HISTORY_CACHE.get_or_init(|| Mutex::new(None));
    if let Ok(guard) = cache.lock() {
        if let Some((release, fetched_at, versions)) = guard.as_ref() {
            if release == current_release && fetched_at.elapsed() < Duration::from_secs(600) {
                return versions.clone();
            }
        }
    }

    let versions = fetch_historical_versions_uncached(current_release);
    if let Ok(mut guard) = cache.lock() {
        *guard = Some((
            current_release.to_string(),
            Instant::now(),
            versions.clone(),
        ));
    }
    versions
}

fn fetch_historical_versions_uncached(current_release: &str) -> Vec<String> {
    let client = match reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(10))
        .user_agent(format!("cargo-sweep/{}", env!("CARGO_PKG_VERSION")))
        .build()
    {
        Ok(client) => client,
        Err(_) => return Vec::new(),
    };
    let response = match client
        .get("https://api.github.com/repos/rust-lang/rust/releases?per_page=20")
        .header("Accept", "application/vnd.github+json")
        .send()
    {
        Ok(response) if response.status().is_success() => response,
        _ => return Vec::new(),
    };
    let Ok(releases) = response.json::<serde_json::Value>() else {
        return Vec::new();
    };
    let Some(releases) = releases.as_array() else {
        return Vec::new();
    };

    let mut versions = Vec::new();
    for release in releases {
        if release.get("draft").and_then(serde_json::Value::as_bool) == Some(true)
            || release
                .get("prerelease")
                .and_then(serde_json::Value::as_bool)
                == Some(true)
        {
            continue;
        }
        let Some(tag) = release.get("tag_name").and_then(serde_json::Value::as_str) else {
            continue;
        };
        if tag == current_release || parse_stable_release(tag).is_none() {
            continue;
        }
        if !versions.iter().any(|version| version == tag) {
            versions.push(tag.to_string());
        }
    }
    versions.sort_by_key(|version| std::cmp::Reverse(parse_stable_release(version)));
    versions.truncate(8);
    versions
}

fn parse_stable_release(value: &str) -> Option<(u32, u32, u32)> {
    let mut parts = value.split('.');
    let major = parts.next()?.parse::<u32>().ok()?;
    let minor = parts.next()?.parse::<u32>().ok()?;
    let patch = parts.next()?.parse::<u32>().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some((major, minor, patch))
}

fn validate_target(target: &str) -> Result<String, String> {
    let target = target.trim();
    if target.is_empty() || target.len() > 80 {
        return Err("工具链名称无效。".into());
    }
    if !target
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_'))
    {
        return Err("工具链名称只能包含字母、数字、点、横线和下划线。".into());
    }
    Ok(target.to_string())
}

#[derive(Default, Clone)]
struct DownloadMeter {
    bytes: u64,
    speed_bps: u64,
}

struct DownloadMonitor {
    meter: Arc<Mutex<DownloadMeter>>,
    stop: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
}

impl DownloadMonitor {
    fn start() -> Self {
        let meter = Arc::new(Mutex::new(DownloadMeter::default()));
        let stop = Arc::new(AtomicBool::new(false));
        let thread_meter = Arc::clone(&meter);
        let thread_stop = Arc::clone(&stop);
        let thread = thread::spawn(move || {
            let mut last_bytes = downloaded_bytes();
            let mut last_time = Instant::now();
            while !thread_stop.load(Ordering::Relaxed) {
                thread::sleep(Duration::from_millis(250));
                let bytes = downloaded_bytes();
                let now = Instant::now();
                let elapsed = now.duration_since(last_time);
                let speed = if elapsed.as_secs_f64() > 0.0 {
                    ((bytes.saturating_sub(last_bytes)) as f64 / elapsed.as_secs_f64()) as u64
                } else {
                    0
                };
                if let Ok(mut meter) = thread_meter.lock() {
                    meter.bytes = bytes;
                    if speed > 0 {
                        meter.speed_bps = speed;
                    } else if elapsed.as_secs() >= 2 {
                        meter.speed_bps = 0;
                    }
                }
                last_bytes = bytes;
                last_time = now;
            }
        });
        Self {
            meter,
            stop,
            thread: Some(thread),
        }
    }

    fn meter(&self) -> Arc<Mutex<DownloadMeter>> {
        Arc::clone(&self.meter)
    }
}

impl Drop for DownloadMonitor {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn downloaded_bytes() -> u64 {
    let Some(home) = env::var_os("RUSTUP_HOME")
        .map(PathBuf::from)
        .or_else(|| dirs::home_dir().map(|home| home.join(".rustup")))
    else {
        return 0;
    };
    let Ok(entries) = fs::read_dir(home.join("downloads")) else {
        return 0;
    };
    entries
        .flatten()
        .filter_map(|entry| entry.metadata().ok())
        .filter(|metadata| metadata.is_file())
        .map(|metadata| metadata.len())
        .sum()
}

#[derive(Default)]
struct TrackedComponent {
    name: String,
    phase: String,
    percent: u8,
    started: Option<Instant>,
    elapsed_ms: u64,
    speed_bps: u64,
}

struct RustupProgress {
    percent: u8,
    downloads: u8,
    installs: u8,
    phase: String,
    message: String,
    components: Vec<TrackedComponent>,
    started: Instant,
    meter: Arc<Mutex<DownloadMeter>>,
}

impl RustupProgress {
    fn new(meter: Arc<Mutex<DownloadMeter>>) -> Self {
        Self {
            percent: 0,
            downloads: 0,
            installs: 0,
            phase: "verify".into(),
            message: "准备安装".into(),
            components: Vec::new(),
            started: Instant::now(),
            meter,
        }
    }

    fn meter_values(&self) -> DownloadMeter {
        self.meter
            .lock()
            .map(|meter| meter.clone())
            .unwrap_or_default()
    }
}

impl RustupProgress {
    fn observe(&mut self, line: &str) -> OperationProgress {
        let clean = strip_ansi(line);
        let lower = clean.to_ascii_lowercase();
        if lower.contains("syncing channel updates") {
            self.phase = "verify".into();
            self.percent = self.percent.max(8);
            self.message = "验证版本与组件信息".into();
        } else if lower.contains("downloading component") {
            self.finish_downloading();
            let name = component_name(&clean);
            self.downloads = self.downloads.saturating_add(1);
            self.phase = "download".into();
            self.percent = self.percent.max(12 + self.downloads.min(6) * 6);
            self.message = format!("正在下载 {name}");
            self.components.push(TrackedComponent {
                name,
                phase: "downloading".into(),
                percent: 0,
                started: Some(Instant::now()),
                elapsed_ms: 0,
                speed_bps: 0,
            });
        } else if lower.contains("installing component") {
            self.finish_downloading();
            let name = component_name(&clean);
            self.installs = self.installs.saturating_add(1);
            self.phase = "install".into();
            self.percent = self.percent.max(52 + self.installs.min(6) * 7);
            self.message = format!("正在安装 {name}");
            let total = self.components.len().max(self.installs as usize).max(1);
            let component_percent = ((self.installs as usize * 100) / total).min(99) as u8;
            if let Some(component) = self
                .components
                .iter_mut()
                .find(|component| component.name == name)
            {
                component.phase = "installing".into();
                component.percent = component_percent;
                component.started = Some(Instant::now());
            } else {
                self.components.push(TrackedComponent {
                    name,
                    phase: "installing".into(),
                    percent: component_percent,
                    started: Some(Instant::now()),
                    elapsed_ms: 0,
                    speed_bps: 0,
                });
            }
        } else if lower.contains("default toolchain set") {
            self.mark_all_done();
            self.phase = "finalize".into();
            self.percent = self.percent.max(96);
            self.message = "设置默认工具链".into();
        } else if lower.contains("checking for self-update") {
            self.phase = "verify".into();
            self.percent = self.percent.max(4);
            self.message = "检查 rustup 更新".into();
        } else if let Some(percent) = line_percentage(&clean) {
            self.percent = self.percent.max(percent.clamp(1, 99));
            self.message = clean.trim().to_string();
        }
        self.snapshot(Some(clean))
    }

    fn finish_downloading(&mut self) {
        let now = Instant::now();
        let speed = self.meter_values().speed_bps;
        for component in &mut self.components {
            if component.phase == "downloading" {
                component.phase = "done".into();
                component.percent = 100;
                component.speed_bps = speed;
                if let Some(started) = component.started {
                    component.elapsed_ms = now.duration_since(started).as_millis() as u64;
                }
            }
        }
    }

    fn mark_all_done(&mut self) {
        let now = Instant::now();
        for component in &mut self.components {
            if component.phase != "done" {
                component.phase = "done".into();
                component.percent = 100;
                if let Some(started) = component.started {
                    component.elapsed_ms = now.duration_since(started).as_millis() as u64;
                }
            }
        }
    }

    fn finish(&mut self, message: &str) -> OperationProgress {
        self.mark_all_done();
        self.phase = "done".into();
        self.percent = 100;
        self.message = message.to_string();
        self.snapshot(None)
    }

    fn snapshot(&self, line: Option<String>) -> OperationProgress {
        let now = Instant::now();
        let meter = self.meter_values();
        let components = self
            .components
            .iter()
            .map(|component| {
                let running = component.phase == "downloading" || component.phase == "installing";
                let elapsed_ms = if running {
                    component
                        .started
                        .map(|started| now.duration_since(started).as_millis() as u64)
                        .unwrap_or(component.elapsed_ms)
                } else {
                    component.elapsed_ms
                };
                ComponentProgress {
                    name: component.name.clone(),
                    phase: component.phase.clone(),
                    percent: component.percent,
                    elapsed_ms,
                    speed_bps: if component.phase == "downloading" {
                        meter.speed_bps
                    } else {
                        component.speed_bps
                    },
                    indeterminate: component.phase == "downloading",
                }
            })
            .collect();
        OperationProgress {
            percent: self.percent,
            message: self.message.clone(),
            phase: self.phase.clone(),
            line,
            components,
            speed_bps: meter.speed_bps,
            elapsed_ms: now.duration_since(self.started).as_millis() as u64,
            downloaded_bytes: meter.bytes,
        }
    }
}

fn component_name(line: &str) -> String {
    line.split("component")
        .nth(1)
        .unwrap_or_default()
        .trim()
        .trim_start_matches('\'')
        .split(|character: char| character == '\'' || character == '(' || character.is_whitespace())
        .next()
        .filter(|name| !name.is_empty())
        .unwrap_or("组件")
        .to_string()
}

fn line_percentage(line: &str) -> Option<u8> {
    line.split_whitespace()
        .find_map(|token| token.strip_suffix('%')?.parse::<u8>().ok())
}

fn run_streaming<F>(program: &str, args: &[&str], on_line: F) -> Result<(bool, String), String>
where
    F: FnMut(&str),
{
    run_streaming_with_env(program, args, &[], on_line)
}

fn run_streaming_with_env<F>(
    program: &str,
    args: &[&str],
    remove_env: &[&str],
    mut on_line: F,
) -> Result<(bool, String), String>
where
    F: FnMut(&str),
{
    let mut command = Command::new(program);
    command.args(args);
    for name in remove_env {
        command.env_remove(name);
    }
    apply_no_window(&mut command);
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command.spawn().map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            format!("未找到 {program}，请确认 Rust 已安装并加入 PATH。")
        } else {
            format!("无法运行 {program}：{error}")
        }
    })?;
    let task_control = background::current_control();
    if let Some(control) = &task_control {
        control.set_pid(child.id());
    }
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| format!("无法读取 {program} 输出。"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| format!("无法读取 {program} 错误输出。"))?;
    let (sender, receiver) = mpsc::channel::<String>();
    let stdout_sender = sender.clone();
    let stdout_reader = thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if stdout_sender.send(line).is_err() {
                break;
            }
        }
    });
    let stderr_sender = sender.clone();
    let stderr_reader = thread::spawn(move || {
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            if stderr_sender.send(line).is_err() {
                break;
            }
        }
    });
    drop(sender);

    let mut text = String::new();
    let mut cancelled = false;
    loop {
        if background::current_cancelled() {
            cancelled = true;
            let _ = child.kill();
            break;
        }
        match receiver.recv_timeout(Duration::from_millis(400)) {
            Ok(line) => {
                if line.is_empty() {
                    continue;
                }
                on_line(&line);
                text.push_str(&line);
                text.push('\n');
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => on_line(""),
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
    let _ = stdout_reader.join();
    let _ = stderr_reader.join();
    let status = child
        .wait()
        .map_err(|error| format!("等待 {program} 退出失败：{error}"))?;
    if let Some(control) = &task_control {
        control.clear_pid();
    }
    let success = status.success() && !cancelled;
    let text = if cancelled {
        clean_output(&format!("{text}\n任务已取消"))
    } else {
        clean_output(&text)
    };
    if !success && text.trim().is_empty() {
        return Err(format!("{program} 执行失败，退出码 {:?}", status.code()));
    }
    Ok((success, text))
}

fn run(program: &str, args: &[&str]) -> Result<String, String> {
    let (_, output) = run_capture(program, args)?;
    Ok(output.trim().to_string())
}

fn run_capture(program: &str, args: &[&str]) -> Result<(bool, String), String> {
    let mut command = Command::new(program);
    command.args(args);
    apply_no_window(&mut command);
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let output = command.output().map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            format!("未找到 {program}，请确认 Rust 已安装并加入 PATH。")
        } else {
            format!("无法运行 {program}：{error}")
        }
    })?;
    let success = output.status.success();
    let text = clean_output(&format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    ));
    if !success && text.trim().is_empty() {
        return Err(format!(
            "{program} 执行失败，退出码 {:?}",
            output.status.code()
        ));
    }
    Ok((success, text))
}

fn apply_no_window(command: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
}

fn parse_rustc_verbose(output: &str) -> (String, String, Option<String>) {
    let mut release = None;
    let mut host = None;
    let mut first = String::new();

    for line in output.lines() {
        let line = line.trim();
        if first.is_empty() && line.starts_with("rustc ") {
            first = line.to_string();
        }
        if let Some(value) = line.strip_prefix("release:") {
            let value = value.trim();
            if is_version_like(value) {
                release = Some(value.to_string());
            }
        }
        if let Some(value) = line.strip_prefix("host:") {
            host = Some(value.trim().to_string());
        }
    }

    let header_version = first
        .split_whitespace()
        .nth(1)
        .filter(|value| is_version_like(value))
        .map(ToOwned::to_owned);
    let version = release.or(header_version).unwrap_or_default();
    let rustc_version = if !first.is_empty() {
        first
    } else if !version.is_empty() {
        format!("rustc {version}")
    } else {
        String::new()
    };

    (rustc_version, version, host)
}

fn parse_active_toolchain(output: &str) -> Option<String> {
    output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .find_map(|line| {
            let token = line.split_whitespace().next()?;
            is_toolchain_name(token).then(|| token.to_string())
        })
}

fn is_toolchain_name(value: &str) -> bool {
    if value.is_empty()
        || value.starts_with("warning:")
        || value.starts_with("error:")
        || value.starts_with("info:")
        || value.ends_with(':')
    {
        return false;
    }
    value.starts_with("stable-")
        || value.starts_with("beta-")
        || value.starts_with("nightly-")
        || value.starts_with("custom-")
        || value
            .chars()
            .next()
            .is_some_and(|character| character.is_ascii_digit())
        || value.contains('-') && value.chars().any(|character| character.is_ascii_digit())
}

fn is_version_like(value: &str) -> bool {
    let release = value.split_whitespace().next().unwrap_or_default();
    let core = release.split('-').next().unwrap_or_default();
    let mut parts = core.split('.');
    let major = parts.next().unwrap_or_default();
    let minor = parts.next().unwrap_or_default();
    !major.is_empty()
        && !minor.is_empty()
        && major.chars().all(|character| character.is_ascii_digit())
        && minor.chars().all(|character| character.is_ascii_digit())
}

fn first_prefixed_line(output: &str, prefix: &str) -> Option<String> {
    output
        .lines()
        .map(str::trim)
        .find(|line| line.starts_with(prefix))
        .map(ToOwned::to_owned)
}

fn channel_from_toolchain(toolchain: &str) -> String {
    let channel = toolchain.split('-').next().unwrap_or(toolchain);
    if channel == "stable" || channel == "beta" || channel == "nightly" {
        return channel.to_string();
    }
    if channel.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        return "pinned".into();
    }
    "custom".into()
}

fn strip_prefix(value: &str, prefix: &str) -> String {
    value.strip_prefix(prefix).unwrap_or(value).to_string()
}

fn clean_output(output: &str) -> String {
    strip_ansi(output)
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .trim()
        .to_string()
}

fn strip_ansi(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(character) = chars.next() {
        if character == '\u{1b}' {
            if chars.peek() == Some(&'[') {
                chars.next();
                for next in chars.by_ref() {
                    if next.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
            continue;
        }
        result.push(character);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    const RUSTC_VERBOSE: &str = "rustc 1.99.1 (58cf8f9c3 2026-10-13)
binary: rustc
commit-hash: 58cf8f9c35b355199727627d07ae34a4e2846d67
commit-date: 2026-10-13
host: x86_64-pc-windows-msvc
release: 1.99.1
LLVM version: 23.1.1";

    #[test]
    fn parses_rustc_verbose_output() {
        let (rustc, release, host) = parse_rustc_verbose(RUSTC_VERBOSE);
        assert_eq!(release, "1.99.1");
        assert_eq!(host.as_deref(), Some("x86_64-pc-windows-msvc"));
        assert!(rustc.starts_with("rustc 1.99.1"));
    }

    #[test]
    fn parses_active_and_installed_toolchains() {
        let active = parse_active_toolchain("stable-x86_64-pc-windows-msvc (default)\n");
        assert_eq!(active.as_deref(), Some("stable-x86_64-pc-windows-msvc"));
        let noisy = parse_active_toolchain(
            "the active toolchain is not installed\nstable-x86_64-pc-windows-msvc (default)\n",
        );
        assert_eq!(noisy.as_deref(), Some("stable-x86_64-pc-windows-msvc"));
        assert!(parse_active_toolchain("the active toolchain\n").is_none());
        assert!(!is_version_like("the"));
        let (_, version, _) = parse_rustc_verbose("the active toolchain is not installed\n");
        assert!(version.is_empty());
        let channel = channel_from_toolchain("nightly-x86_64-pc-windows-msvc");
        assert_eq!(channel, "nightly");
        let pinned = channel_from_toolchain("1.94.1-x86_64-pc-windows-msvc");
        assert_eq!(pinned, "pinned");
    }

    #[test]
    fn parses_download_progress_from_rustup_output() {
        let mut progress = RustupProgress::new(Arc::new(Mutex::new(DownloadMeter::default())));
        assert_eq!(
            progress
                .observe("info: syncing channel updates for 'stable-x86_64-pc-windows-msvc'")
                .percent,
            8
        );
        let download = progress.observe("info: downloading component 'rustc'");
        assert!(download.message.contains("rustc"));
        assert!(download.percent > 8);
        assert_eq!(download.components[0].phase, "downloading");
        let json = serde_json::to_value(&download).unwrap();
        assert_eq!(json["components"][0]["name"], "rustc");
        assert_eq!(json["components"][0]["indeterminate"], true);
        assert!(json["speedBps"].is_number());
        let install = progress.observe("info: installing component 'rustc'");
        assert!(install.percent > download.percent);
        assert_eq!(install.components[0].phase, "installing");
        assert_eq!(
            progress
                .observe("info: default toolchain set to 'stable'")
                .percent,
            96
        );
        let line = progress.observe("unrelated output");
        assert_eq!(line.line.as_deref(), Some("unrelated output"));
    }

    #[test]
    fn streams_command_output_without_deadlock() {
        let mut lines = 0usize;
        let (success, text) = run_streaming("rustc", &["--version"], |_| {
            lines += 1;
        })
        .unwrap();
        assert!(success);
        assert!(text.contains("rustc"));
        assert!(lines > 0);
    }

    #[test]
    fn parses_percentage_lines_from_command_output() {
        assert_eq!(line_percentage("downloading 42%"), Some(42));
        assert_eq!(line_percentage("no percentage here"), None);
    }

    #[test]
    fn splits_manifest_versions_and_history() {
        let (release, date) = split_manifest_version("1.99.1 (58cf8f9c3 2026-10-13)");
        assert_eq!(release, "1.99.1");
        assert_eq!(date.as_deref(), Some("2026-10-13"));
        assert_eq!(parse_stable_release("1.99.1"), Some((1, 99, 1)));
        assert_eq!(parse_stable_release("1.98.0"), Some((1, 98, 0)));
        assert_eq!(parse_stable_release("1.98"), None);
        assert_eq!(parse_stable_release("1.98.0-beta.1"), None);
    }

    #[test]
    fn detects_toolchain_repair_errors() {
        assert!(needs_toolchain_repair(
            "could not rename component file: 目录不是空的 (os error 145)"
        ));
        assert!(needs_toolchain_repair(
            "failed to install component: detected conflict: 'bin\\rust-gdb'"
        ));
        assert!(!needs_toolchain_repair(
            "error: could not download file: http request returned 404"
        ));
    }

    #[test]
    fn detects_the_current_toolchain() {
        let status = detect().expect("rustc should be available in the test environment");
        assert!(status.installed);
        assert!(status.version.is_some());
        assert!(status.rustup_installed);
        assert!(status.rustup_path.is_some());
        assert!(status
            .channels
            .iter()
            .any(|channel| channel.channel == "stable"));
    }

    #[test]
    #[ignore = "requires access to static.rust-lang.org"]
    fn fetches_latest_channels_from_rust_lang() {
        let latest = latest().expect("latest versions should be reachable");
        assert!(latest.stable.is_some());
        assert!(latest.nightly.is_some());
        assert!(!latest.historical_versions.is_empty());
    }

    #[test]
    fn strips_ansi_sequences_from_rustup_output() {
        assert_eq!(
            strip_ansi("\u{1b}[32minfo\u{1b}[0m: updated\r\n"),
            "info: updated\r\n"
        );
        assert_eq!(
            clean_output("\u{1b}[32minfo\u{1b}[0m: updated\r\n"),
            "info: updated"
        );
    }
}
