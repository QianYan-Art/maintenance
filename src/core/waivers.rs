use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::core::normalize_project;

const WAIVERS_DIR: &str = ".doc-maintenance";
const WAIVERS_FILE: &str = "waivers.toml";

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct WaiverFile {
    #[serde(default)]
    pub(crate) waiver: Vec<Waiver>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Waiver {
    pub(crate) token: String,
    pub(crate) reason: String,
    pub(crate) date: String,
}

#[derive(Debug)]
pub(crate) struct WaiveOutcome {
    pub(crate) path: PathBuf,
    pub(crate) added: bool,
}

impl WaiverFile {
    pub(crate) fn contains(&self, token: &str) -> bool {
        self.waiver.iter().any(|waiver| waiver.token == token)
    }

    pub(crate) fn tokens(&self) -> BTreeSet<String> {
        self.waiver
            .iter()
            .map(|waiver| waiver.token.clone())
            .collect()
    }

    pub(crate) fn reason_for(&self, token: &str) -> Option<&str> {
        self.waiver
            .iter()
            .find(|waiver| waiver.token == token)
            .map(|waiver| waiver.reason.as_str())
    }
}

pub(crate) fn load_waivers(project: &Path) -> Result<WaiverFile, String> {
    let path = waivers_path(project)?;
    if !path.exists() {
        return Ok(WaiverFile::default());
    }
    let text = fs::read_to_string(&path)
        .map_err(|error| format!("cannot read waivers {}: {error}", path.display()))?;
    toml::from_str(&text).map_err(|error| format!("invalid waivers {}: {error}", path.display()))
}

pub(crate) fn add_waiver(
    project: &Path,
    token: &str,
    reason: &str,
) -> Result<WaiveOutcome, String> {
    let token = token.trim();
    let reason = reason.trim();
    if token.is_empty() {
        return Err("waive token must not be empty".to_string());
    }
    if reason.is_empty() {
        return Err("waive reason must not be empty".to_string());
    }

    let path = waivers_path(project)?;
    let mut waivers = load_waivers(project)?;
    if waivers.contains(token) {
        return Ok(WaiveOutcome { path, added: false });
    }
    waivers.waiver.push(Waiver {
        token: token.to_string(),
        reason: reason.to_string(),
        date: today_utc(),
    });
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "cannot create waivers directory {}: {error}",
                parent.display()
            )
        })?;
    }
    let text = toml::to_string_pretty(&waivers)
        .map_err(|error| format!("cannot render waivers {}: {error}", path.display()))?;
    fs::write(&path, text)
        .map_err(|error| format!("cannot write waivers {}: {error}", path.display()))?;
    Ok(WaiveOutcome { path, added: true })
}

fn waivers_path(project: &Path) -> Result<PathBuf, String> {
    Ok(normalize_project(project)?
        .join(WAIVERS_DIR)
        .join(WAIVERS_FILE))
}

fn today_utc() -> String {
    let days = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64 / 86_400)
        .unwrap_or(0);
    let (year, month, day) = civil_from_days(days);
    format!("{year:04}-{month:02}-{day:02}")
}

fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let days = days + 719_468;
    let era = if days >= 0 { days } else { days - 146_096 } / 146_097;
    let day_of_era = days - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    let year = year + if month <= 2 { 1 } else { 0 };
    (year, month, day)
}
