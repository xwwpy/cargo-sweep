use serde::Serialize;
use std::process::{Command, Stdio};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledToolchain {
    pub name: String,
    pub active: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RustToolchainStatus {
    pub version: String,
    pub rustc_version: String,
    pub release: String,
    pub host: Option<String>,
    pub channel: Option<String>,
    pub active_toolchain: Option<String>,
    pub toolchains: Vec<InstalledToolchain>,
    pub cargo_version: Option<String>,
    pub rustup_version: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolchainUpdateResult {
    pub success: bool,
    pub output: String,
    pub status: RustToolchainStatus,
}

pub fn detect() -> Result<RustToolchainStatus, String> {
    let verbose = run("rustc", &["--version", "--verbose"])?;
    let (rustc_version, release, host) = parse_rustc_verbose(&verbose);

    let active_toolchain = run("rustup", &["show", "active-toolchain"])
        .ok()
        .and_then(|output| parse_active_toolchain(&output));
    let toolchains = run("rustup", &["toolchain", "list"])
        .ok()
        .map(|output| parse_toolchains(&output, active_toolchain.as_deref()))
        .unwrap_or_default();
    let cargo_version = run("cargo", &["--version"])
        .ok()
        .and_then(|output| first_line(&output))
        .map(|version| {
            version
                .strip_prefix("cargo ")
                .unwrap_or(&version)
                .to_string()
        });
    let rustup_version = run("rustup", &["--version"])
        .ok()
        .and_then(|output| first_line(&output))
        .map(|version| {
            version
                .strip_prefix("rustup ")
                .unwrap_or(&version)
                .to_string()
        });
    let channel = active_toolchain.as_deref().and_then(channel_from_toolchain);

    Ok(RustToolchainStatus {
        version: release.clone(),
        rustc_version,
        release,
        host,
        channel,
        active_toolchain,
        toolchains,
        cargo_version,
        rustup_version,
    })
}

pub fn update() -> Result<ToolchainUpdateResult, String> {
    let before = detect()?;
    let mut command = Command::new("rustup");
    command.arg("update").arg("--no-self-update");
    if let Some(toolchain) = before.active_toolchain.as_deref() {
        command.arg(toolchain);
    }
    apply_no_window(&mut command);
    command.stdout(Stdio::piped()).stderr(Stdio::piped());

    let output = command
        .output()
        .map_err(|error| format!("无法启动 rustup update：{error}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}{stderr}");
    let output_text = clean_output(&combined);

    if !output.status.success() && output_text.trim().is_empty() {
        return Err(format!(
            "rustup update 失败，退出码 {:?}",
            output.status.code()
        ));
    }

    let status = detect().unwrap_or(before);
    Ok(ToolchainUpdateResult {
        success: output.status.success(),
        output: if output_text.trim().is_empty() {
            "没有新的更新。".into()
        } else {
            output_text.trim().to_string()
        },
        status,
    })
}

fn run(program: &str, args: &[&str]) -> Result<String, String> {
    let mut command = Command::new(program);
    command.args(args);
    apply_no_window(&mut command);
    let output = command.output().map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            format!("未找到 {program}，请确认 Rust 已安装并加入 PATH。")
        } else {
            format!("无法运行 {program}：{error}")
        }
    })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("{program} 执行失败：{}", stderr.trim()));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
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

fn parse_toolchains(output: &str, active: Option<&str>) -> Vec<InstalledToolchain> {
    output
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() {
                return None;
            }
            let name = line.split_whitespace().next()?.to_string();
            let is_active = line.contains("(active") || active == Some(name.as_str());
            Some(InstalledToolchain {
                name,
                active: is_active,
            })
        })
        .collect()
}

fn channel_from_toolchain(toolchain: &str) -> Option<String> {
    let channel = toolchain.split('-').next()?;
    if channel.is_empty() {
        return None;
    }
    if channel == "stable" || channel == "beta" || channel == "nightly" {
        return Some(channel.to_string());
    }
    if channel.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        return Some("pinned".into());
    }
    Some("custom".into())
}

fn first_line(output: &str) -> Option<String> {
    output
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(ToOwned::to_owned)
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

        let toolchains = parse_toolchains(
            "stable-x86_64-pc-windows-msvc (active, default)\nnightly-x86_64-pc-windows-msvc\n",
            active.as_deref(),
        );
        assert_eq!(toolchains.len(), 2);
        assert!(toolchains[0].active);
        assert!(!toolchains[1].active);
    }

    #[test]
    fn identifies_toolchain_channels() {
        assert_eq!(
            channel_from_toolchain("stable-x86_64-pc-windows-msvc").as_deref(),
            Some("stable")
        );
        assert_eq!(
            channel_from_toolchain("1.94.1-x86_64-pc-windows-msvc").as_deref(),
            Some("pinned")
        );
    }

    #[test]
    fn detects_the_current_toolchain() {
        let status = detect().expect("rustc should be available in the test environment");
        assert!(!status.version.is_empty());
        assert!(status.rustc_version.starts_with("rustc"));
        assert!(status
            .release
            .starts_with(|character: char| character.is_ascii_digit()));
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
