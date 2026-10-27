use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};
use toml_edit::{value, DocumentMut, Item, Table};

pub const OFFICIAL_INDEX: &str = "sparse+https://index.crates.io/";
const MANAGED_SOURCE: &str = "cargo-sweep-managed";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NamedSource {
    pub name: String,
    pub index: String,
    pub kind: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CargoSourceStatus {
    pub cargo_home: String,
    pub config_path: String,
    pub config_dir: String,
    pub config_exists: bool,
    pub cargo_version: Option<String>,
    pub current_name: String,
    pub current_index: String,
    pub current_is_official: bool,
    pub replace_with: Option<String>,
    pub configured_sources: Vec<NamedSource>,
    pub warning: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplySourceResult {
    pub backup_path: Option<String>,
    pub status: CargoSourceStatus,
}

pub fn detect() -> Result<CargoSourceStatus, String> {
    let cargo_home = cargo_home()?;
    let config_path = config_path(&cargo_home);
    let config_exists = config_path.is_file();
    let mut warning = None;

    if cargo_home.join("config").is_file() && cargo_home.join("config.toml").is_file() {
        warning = Some("config 与 config.toml 同时存在，Cargo 将优先使用 config.toml。".into());
    }

    let contents = if config_exists {
        fs::read_to_string(&config_path).map_err(|error| format!("无法读取 Cargo 配置：{error}"))?
    } else {
        String::new()
    };

    let parsed: Option<toml::Value> = if contents.trim().is_empty() {
        None
    } else {
        Some(
            toml::from_str(&contents)
                .map_err(|error| format!("Cargo 配置不是合法的 TOML：{error}"))?,
        )
    };

    let mut configured_sources = Vec::new();
    let mut replace_with = None;
    let mut current_name = "crates.io".to_string();
    let mut current_index = OFFICIAL_INDEX.to_string();
    let mut current_is_official = true;

    if let Some(root) = parsed.as_ref() {
        if let Some(sources) = root.get("source").and_then(toml::Value::as_table) {
            replace_with = sources
                .get("crates-io")
                .and_then(|item| item.get("replace-with"))
                .and_then(toml::Value::as_str)
                .map(ToOwned::to_owned);

            for (name, source) in sources {
                if name == "crates-io" {
                    continue;
                }
                if let Some(index) = source.get("registry").and_then(toml::Value::as_str) {
                    configured_sources.push(NamedSource {
                        name: name.clone(),
                        index: index.to_string(),
                        kind: "registry".into(),
                    });
                } else if let Some(index) =
                    source.get("local-registry").and_then(toml::Value::as_str)
                {
                    configured_sources.push(NamedSource {
                        name: name.clone(),
                        index: index.to_string(),
                        kind: "local-registry".into(),
                    });
                } else if let Some(index) = source.get("directory").and_then(toml::Value::as_str) {
                    configured_sources.push(NamedSource {
                        name: name.clone(),
                        index: index.to_string(),
                        kind: "directory".into(),
                    });
                }
            }

            if let Some(name) = replace_with.as_ref() {
                current_name = name.clone();
                current_is_official = false;
                if let Some(source) = sources.get(name) {
                    current_index = source
                        .get("registry")
                        .and_then(toml::Value::as_str)
                        .or_else(|| source.get("local-registry").and_then(toml::Value::as_str))
                        .or_else(|| source.get("directory").and_then(toml::Value::as_str))
                        .unwrap_or_default()
                        .to_string();
                } else {
                    current_index = String::new();
                    warning = Some(format!("配置引用了不存在的 source.{name}。"));
                }
            } else if let Some(index) = sources
                .get("crates-io")
                .and_then(|item| item.get("registry"))
                .and_then(toml::Value::as_str)
            {
                current_index = index.to_string();
                current_is_official = false;
            }
        }

        if let Some(registries) = root.get("registries").and_then(toml::Value::as_table) {
            for (name, registry) in registries {
                if let Some(index) = registry.get("index").and_then(toml::Value::as_str) {
                    configured_sources.push(NamedSource {
                        name: name.clone(),
                        index: index.to_string(),
                        kind: "registries".into(),
                    });
                }
            }
        }
    }

    Ok(CargoSourceStatus {
        cargo_home: cargo_home.display().to_string(),
        config_path: config_path.display().to_string(),
        config_dir: config_path
            .parent()
            .map(|path| path.display().to_string())
            .unwrap_or_default(),
        config_exists,
        cargo_version: cargo_version(),
        current_name,
        current_index,
        current_is_official,
        replace_with,
        configured_sources,
        warning,
    })
}

pub fn apply(url: String) -> Result<ApplySourceResult, String> {
    let index_url = normalize_index_url(&url)?;
    let cargo_home = cargo_home()?;
    let config_path = config_path(&cargo_home);
    let is_official = index_url == OFFICIAL_INDEX;

    if is_official && !config_path.exists() {
        return Ok(ApplySourceResult {
            backup_path: None,
            status: detect()?,
        });
    }

    let contents = if config_path.exists() {
        fs::read_to_string(&config_path).map_err(|error| format!("无法读取 Cargo 配置：{error}"))?
    } else {
        String::new()
    };
    let mut document = if contents.trim().is_empty() {
        DocumentMut::new()
    } else {
        contents
            .parse::<DocumentMut>()
            .map_err(|error| format!("Cargo 配置不是合法的 TOML：{error}"))?
    };

    if is_official {
        reset_to_official(&mut document)?;
    } else {
        set_managed_source(&mut document, &index_url)?;
    }

    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("无法创建 Cargo 配置目录：{error}"))?;
    }

    let backup_path = backup_config(&config_path)?;
    fs::write(&config_path, document.to_string())
        .map_err(|error| format!("无法写入 Cargo 配置：{error}"))?;

    Ok(ApplySourceResult {
        backup_path: backup_path.map(|path| path.display().to_string()),
        status: detect()?,
    })
}

fn set_managed_source(document: &mut DocumentMut, index_url: &str) -> Result<(), String> {
    let root = document.as_table_mut();
    if !root.contains_key("source") {
        root.insert("source", Item::Table(Table::new()));
    }
    let source = root
        .get_mut("source")
        .and_then(Item::as_table_mut)
        .ok_or_else(|| "配置中的 [source] 不是表，无法修改。".to_string())?;

    if !source.contains_key("crates-io") {
        source.insert("crates-io", Item::Table(Table::new()));
    }
    let crates_io = source
        .get_mut("crates-io")
        .and_then(Item::as_table_mut)
        .ok_or_else(|| "配置中的 [source.crates-io] 不是表，无法修改。".to_string())?;
    crates_io.insert("replace-with", value(MANAGED_SOURCE));

    if !source.contains_key(MANAGED_SOURCE) {
        source.insert(MANAGED_SOURCE, Item::Table(Table::new()));
    }
    let managed = source
        .get_mut(MANAGED_SOURCE)
        .and_then(Item::as_table_mut)
        .ok_or_else(|| format!("配置中的 [source.{MANAGED_SOURCE}] 不是表，无法修改。"))?;
    managed.insert("registry", value(index_url));
    Ok(())
}

fn reset_to_official(document: &mut DocumentMut) -> Result<(), String> {
    let root = document.as_table_mut();
    if let Some(source) = root.get_mut("source").and_then(Item::as_table_mut) {
        if let Some(crates_io) = source.get_mut("crates-io").and_then(Item::as_table_mut) {
            crates_io.remove("replace-with");
        }
        source.remove(MANAGED_SOURCE);
    }
    Ok(())
}

fn normalize_index_url(input: &str) -> Result<String, String> {
    let url = input.trim();
    if url.is_empty() {
        return Err("请输入 Cargo 索引地址。".into());
    }
    if url.starts_with("sparse+http://") || url.starts_with("http://") {
        return Err("Cargo 源必须使用 HTTPS，不能使用 HTTP。".into());
    }
    if !(url.starts_with("sparse+https://") || url.starts_with("https://")) {
        return Err("索引地址需以 sparse+https:// 或 https:// 开头。".into());
    }
    if url.chars().any(char::is_whitespace) {
        return Err("索引地址不能包含空白字符。".into());
    }
    Ok(url.to_string())
}

fn cargo_home() -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os("CARGO_HOME") {
        return Ok(PathBuf::from(path));
    }
    dirs::home_dir()
        .map(|home| home.join(".cargo"))
        .ok_or_else(|| "无法定位用户目录。".to_string())
}

fn config_path(cargo_home: &Path) -> PathBuf {
    let toml_path = cargo_home.join("config.toml");
    if toml_path.is_file() {
        toml_path
    } else {
        let legacy = cargo_home.join("config");
        if legacy.is_file() {
            legacy
        } else {
            toml_path
        }
    }
}

fn backup_config(path: &Path) -> Result<Option<PathBuf>, String> {
    if !path.is_file() {
        return Ok(None);
    }
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default();
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("config.toml");
    let mut candidate = path.with_file_name(format!("{name}.cargo-sweep-backup-{seconds}"));
    let mut suffix = 1usize;
    while candidate.exists() {
        candidate = path.with_file_name(format!("{name}.cargo-sweep-backup-{seconds}-{suffix}"));
        suffix += 1;
    }
    fs::copy(path, &candidate).map_err(|error| format!("备份 Cargo 配置失败：{error}"))?;
    Ok(Some(candidate))
}

fn cargo_version() -> Option<String> {
    let mut command = Command::new("cargo");
    command.arg("--version");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let output = command.output().ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn accepts_sparse_and_git_https_sources() {
        assert_eq!(
            normalize_index_url("sparse+https://rsproxy.cn/index/").unwrap(),
            "sparse+https://rsproxy.cn/index/"
        );
        assert_eq!(
            normalize_index_url("https://example.com/index").unwrap(),
            "https://example.com/index"
        );
    }

    #[test]
    fn rejects_insecure_or_unknown_urls() {
        assert!(normalize_index_url("http://example.com/index").is_err());
        assert!(normalize_index_url("example.com/index").is_err());
        assert!(normalize_index_url("").is_err());
    }

    #[test]
    fn writes_and_removes_the_managed_source() {
        let mut document = DocumentMut::new();
        set_managed_source(&mut document, "sparse+https://rsproxy.cn/index/").unwrap();
        let text = document.to_string();
        assert!(text.contains("[source.crates-io]"));
        assert!(text.contains("replace-with = \"cargo-sweep-managed\""));
        assert!(text.contains("registry = \"sparse+https://rsproxy.cn/index/\""));

        reset_to_official(&mut document).unwrap();
        let text = document.to_string();
        assert!(!text.contains("replace-with"));
        assert!(!text.contains("cargo-sweep-managed"));
    }

    #[test]
    fn detects_switches_and_backs_up_the_global_config() {
        let temp = tempdir().unwrap();
        let previous_cargo_home = std::env::var_os("CARGO_HOME");
        std::env::set_var("CARGO_HOME", temp.path());

        let config = temp.path().join("config.toml");
        let original = "[net]\ngit-fetch-with-cli = true\n";
        fs::write(&config, original).unwrap();

        let initial = detect().unwrap();
        assert!(initial.current_is_official);
        assert_eq!(initial.current_index, OFFICIAL_INDEX);
        assert!(initial.config_exists);

        let applied = apply("sparse+https://rsproxy.cn/index/".into()).unwrap();
        assert!(applied.backup_path.is_some());
        assert_eq!(
            applied.status.current_index,
            "sparse+https://rsproxy.cn/index/"
        );
        assert!(!applied.status.current_is_official);
        let content = fs::read_to_string(&config).unwrap();
        assert!(content.contains("[net]"));
        assert!(content.contains("replace-with = \"cargo-sweep-managed\""));
        assert!(content.contains("sparse+https://rsproxy.cn/index/"));
        assert_eq!(
            fs::read_to_string(applied.backup_path.unwrap()).unwrap(),
            original
        );

        let official = apply(OFFICIAL_INDEX.into()).unwrap();
        assert!(official.status.current_is_official);
        assert!(official.backup_path.is_some());
        let content = fs::read_to_string(&config).unwrap();
        assert!(content.contains("[net]"));
        assert!(!content.contains("replace-with"));
        assert!(!content.contains("cargo-sweep-managed"));

        match previous_cargo_home {
            Some(value) => std::env::set_var("CARGO_HOME", value),
            None => std::env::remove_var("CARGO_HOME"),
        }
    }
}
