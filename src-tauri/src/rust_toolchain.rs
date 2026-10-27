use serde::Serialize;
use std::process::{Command, Stdio};
use std::time::Duration;
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
pub struct RustInstallationProbe {
    pub installed: bool,
    pub rustup_installed: bool,
    pub version: Option<String>,
}

pub fn probe() -> RustInstallationProbe {
    let rustc_version = run("rustc", &["--version"])
        .ok()
        .and_then(|output| first_line(&output));
    let rustup_version = run("rustup", &["--version"])
        .ok()
        .and_then(|output| first_line(&output));
    RustInstallationProbe {
        installed: rustc_version.is_some(),
        rustup_installed: rustup_version.is_some(),
        version: rustc_version,
    }
}

pub fn detect() -> Result<RustToolchainStatus, String> {
    let rustup_version = run("rustup", &["--version"])
        .ok()
        .and_then(|output| first_line(&output))
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
        .and_then(|output| first_line(&output))
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
    let historical_versions = stable.as_ref().map(historical_versions).unwrap_or_default();

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

pub fn install(target: String, set_default: bool) -> Result<ToolchainOperationResult, String> {
    let target = validate_target(&target)?;
    let before = detect()?;
    if !before.rustup_installed {
        return Err("未检测到 rustup，请先安装 Rustup 后再下载工具链。".into());
    }

    let mut command = Command::new("rustup");
    command.args(["toolchain", "install", &target, "--profile", "default"]);
    apply_no_window(&mut command);
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let output = command
        .output()
        .map_err(|error| format!("无法启动 rustup：{error}"))?;
    let success = output.status.success();
    let mut text = clean_output(&format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    ));

    if success && (set_default || before.default_toolchain.is_none()) {
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

pub fn update(target: Option<String>) -> Result<ToolchainOperationResult, String> {
    let target = target.map(|value| validate_target(&value)).transpose()?;
    let before = detect()?;
    let toolchain = target
        .or_else(|| before.active_toolchain.clone())
        .or_else(|| before.default_toolchain.clone());

    let mut command = Command::new("rustup");
    command.arg("update").arg("--no-self-update");
    if let Some(toolchain) = toolchain.as_deref() {
        command.arg(toolchain);
    }
    apply_no_window(&mut command);
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let output = command
        .output()
        .map_err(|error| format!("无法启动 rustup update：{error}"))?;
    let success = output.status.success();
    let text = clean_output(&format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    ));
    if !success && text.trim().is_empty() {
        return Err(format!(
            "rustup update 失败，退出码 {:?}",
            output.status.code()
        ));
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

fn historical_versions(stable: &LatestRustToolchain) -> Vec<String> {
    let mut parts = stable.release.split('.');
    let major = parts.next().and_then(|value| value.parse::<u32>().ok());
    let minor = parts.next().and_then(|value| value.parse::<u32>().ok());
    let (Some(major), Some(minor)) = (major, minor) else {
        return Vec::new();
    };
    (0..8)
        .filter_map(|offset| {
            let minor = minor.checked_sub(offset)?;
            Some(format!("{major}.{minor}.0"))
        })
        .collect()
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
            release = Some(value.trim().to_string());
        }
        if let Some(value) = line.strip_prefix("host:") {
            host = Some(value.trim().to_string());
        }
    }

    if first.is_empty() {
        first = output.lines().next().unwrap_or("rustc").trim().to_string();
    }
    let version = release
        .clone()
        .or_else(|| first.split_whitespace().nth(1).map(ToOwned::to_owned))
        .unwrap_or_else(|| "未知".into());
    let rustc_version = if first.is_empty() {
        format!("rustc {version}")
    } else {
        first
    };

    (rustc_version, version, host)
}

fn parse_active_toolchain(output: &str) -> Option<String> {
    output
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .and_then(|line| line.split_whitespace().next())
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

fn first_line(output: &str) -> Option<String> {
    output
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(ToOwned::to_owned)
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
        let channel = channel_from_toolchain("nightly-x86_64-pc-windows-msvc");
        assert_eq!(channel, "nightly");
        let pinned = channel_from_toolchain("1.94.1-x86_64-pc-windows-msvc");
        assert_eq!(pinned, "pinned");
    }

    #[test]
    fn splits_manifest_versions_and_history() {
        let (release, date) = split_manifest_version("1.99.1 (58cf8f9c3 2026-10-13)");
        assert_eq!(release, "1.99.1");
        assert_eq!(date.as_deref(), Some("2026-10-13"));
        let latest = LatestRustToolchain {
            channel: "stable".into(),
            version: "1.99.1 (58cf8f9c3 2026-10-13)".into(),
            release: "1.99.1".into(),
            date: Some("2026-10-13".into()),
        };
        let history = historical_versions(&latest);
        assert!(history.contains(&"1.99.0".to_string()));
        assert!(history.contains(&"1.92.0".to_string()));
    }

    #[test]
    fn detects_the_current_toolchain() {
        let status = detect().expect("rustc should be available in the test environment");
        assert!(status.installed);
        assert!(status.version.is_some());
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
