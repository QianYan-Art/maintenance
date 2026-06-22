use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::core::normalize_project;

const CONFIG_DIR: &str = ".doc-maintenance";
const CONFIG_FILE: &str = "config.toml";

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct ProjectConfig {
    #[serde(default)]
    pub(crate) dev_docs: Vec<PathBuf>,
    #[serde(default)]
    pub(crate) record_docs: Vec<PathBuf>,
    #[serde(default)]
    pub(crate) summary_source: Vec<PathBuf>,
    #[serde(default)]
    pub(crate) topic: Vec<String>,
}

#[derive(Debug)]
pub(crate) struct InitConfigOutcome {
    pub(crate) path: PathBuf,
    pub(crate) created: bool,
}

pub(crate) fn init_project_config(project: &Path) -> Result<InitConfigOutcome, String> {
    let path = project_config_path(project)?;
    if path.exists() {
        return Ok(InitConfigOutcome {
            path,
            created: false,
        });
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "cannot create config directory {}: {error}",
                parent.display()
            )
        })?;
    }
    fs::write(&path, default_config_template())
        .map_err(|error| format!("cannot write config {}: {error}", path.display()))?;
    Ok(InitConfigOutcome {
        path,
        created: true,
    })
}

pub(crate) fn load_project_config(project: &Path) -> Result<ProjectConfig, String> {
    let path = project_config_path(project)?;
    if !path.exists() {
        return Ok(ProjectConfig::default());
    }
    let text = fs::read_to_string(&path)
        .map_err(|error| format!("cannot read config {}: {error}", path.display()))?;
    toml::from_str(&text).map_err(|error| format!("invalid config {}: {error}", path.display()))
}

fn project_config_path(project: &Path) -> Result<PathBuf, String> {
    Ok(normalize_project(project)?
        .join(CONFIG_DIR)
        .join(CONFIG_FILE))
}

fn default_config_template() -> &'static str {
    r#"# Doc Maintenance 本地默认配置。
# 路径默认相对于项目根目录，也可以填写绝对路径。

# 开发文档。留空时自动发现 README.md 与 docs/。
dev_docs = []

# KBase 记录文档。留空时不读取。
record_docs = []

# 可选 summary 来源，用于 route packet 上下文。
summary_source = []

# record_docs 的 topic 过滤。留空时不过滤。
topic = []
"#
}
