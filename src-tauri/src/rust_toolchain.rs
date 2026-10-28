use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
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
    let rustup_path = prepare_rust_environment();
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
        rustup_path: rustup_path.map(|path| path.to_string_lossy().to_string()),
    }
}

pub fn detect() -> Result<RustToolchainStatus, String> {
    crate::cargo_home::apply_saved_homes();
    let rustup_path = prepare_rust_environment();
    let settings = read_settings();
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

pub fn install_rustup() -> Result<ToolchainOperationResult, String> {
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
            ensure_stable_toolchain(&mut text)?;
            let status = detect()?;
            return Ok(ToolchainOperationResult {
                success: true,
                output: text,
                status,
            });
        }
        return Err("winget 已结束，但未检测到 rustup，请检查 winget 输出。".into());
    }

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
    let response = client
        .get(&url)
        .send()
        .map_err(|error| format!("无法下载 rustup-init：{error}"))?;
    if !response.status().is_success() {
        return Err(format!("无法下载 rustup-init：HTTP {}", response.status()));
    }
    let bytes = response
        .bytes()
        .map_err(|error| format!("无法读取 rustup-init 下载内容：{error}"))?;
    if bytes.len() < 1024 * 1024 {
        return Err("下载的 rustup-init 文件不完整，请重试。".into());
    }
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
