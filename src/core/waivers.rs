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
    /// Entries written before scopes existed have no `scope` and stay
    /// project-wide, so old waiver files keep their meaning.
    #[serde(default)]
    pub(crate) scope: WaiverScope,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) run: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) expires: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum WaiverScope {
    #[default]
    Project,
    Run,
}

impl WaiverScope {
    pub(crate) fn parse(value: &str) -> Result<Self, String> {
        match value {
            "project" => Ok(Self::Project),
            "run" => Ok(Self::Run),
            other => Err(format!(
                "invalid waiver scope {other:?}; use \"run\" or \"project\""
            )),
        }
    }
}

#[derive(Debug)]
pub(crate) struct WaiveRequest<'a> {
    pub(crate) token: &'a str,
    pub(crate) reason: &'a str,
    pub(crate) scope: WaiverScope,
    pub(crate) expires: Option<&'a str>,
}

#[derive(Debug)]
pub(crate) struct WaiveOutcome {
    pub(crate) path: PathBuf,
    pub(crate) added: bool,
    pub(crate) run: Option<String>,
}

/// The waivers that apply to one closeout run on one day.
#[derive(Debug, Default)]
pub(crate) struct ActiveWaivers {
    reasons: std::collections::BTreeMap<String, String>,
}

impl ActiveWaivers {
    pub(crate) fn contains(&self, token: &str) -> bool {
        self.reasons.contains_key(token)
    }

    pub(crate) fn tokens(&self) -> BTreeSet<String> {
        self.reasons.keys().cloned().collect()
    }

    pub(crate) fn reason_for(&self, token: &str) -> Option<&str> {
        self.reasons.get(token).map(String::as_str)
    }
}

impl Waiver {
    fn is_active(&self, run_id: Option<&str>, today: &str) -> bool {
        if self
            .expires
            .as_deref()
            .is_some_and(|expires| expires < today)
        {
            return false;
        }
        match self.scope {
            WaiverScope::Project => true,
            WaiverScope::Run => run_id.is_some() && self.run.as_deref() == run_id,
        }
    }
}

impl WaiverFile {
    /// Project waivers apply everywhere until they expire; run waivers apply
    /// only to verify for the closeout run they were recorded against.
    pub(crate) fn active(&self, run_id: Option<&str>) -> ActiveWaivers {
        let today = today_utc();
        let mut reasons = std::collections::BTreeMap::new();
        for waiver in self
            .waiver
            .iter()
            .filter(|waiver| waiver.is_active(run_id, &today))
        {
            reasons
                .entry(waiver.token.clone())
                .or_insert_with(|| waiver.reason.clone());
        }
        ActiveWaivers { reasons }
    }

    fn has_same(&self, token: &str, scope: &WaiverScope, run: Option<&str>) -> bool {
        self.waiver.iter().any(|waiver| {
            waiver.token == token && &waiver.scope == scope && waiver.run.as_deref() == run
        })
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

pub(crate) fn add_waiver(project: &Path, request: WaiveRequest) -> Result<WaiveOutcome, String> {
    let token = request.token.trim();
    let reason = request.reason.trim();
    if token.is_empty() {
        return Err("waive token must not be empty".to_string());
    }
    if reason.is_empty() {
        return Err("waive reason must not be empty".to_string());
    }
    if let Some(expires) = request.expires {
        validate_date(expires)?;
    }

    let normalized = normalize_project(project)?;
    let run = match request.scope {
        WaiverScope::Run => Some(crate::core::latest_closeout_run_id(&normalized)?.ok_or_else(
            || {
                "no closeout run to scope this waiver to; run closeout first, or pass --scope project for a project-wide waiver".to_string()
            },
        )?),
        WaiverScope::Project => None,
    };
    let path = crate::core::ensure_artifact_dir(&normalized)?.join(WAIVERS_FILE);
    let mut waivers = load_waivers(project)?;
    if waivers.has_same(token, &request.scope, run.as_deref()) {
        return Ok(WaiveOutcome {
            path,
            added: false,
            run,
        });
    }
    waivers.waiver.push(Waiver {
        token: token.to_string(),
        reason: reason.to_string(),
        date: today_utc(),
        scope: request.scope,
        run: run.clone(),
        expires: request.expires.map(str::to_string),
    });
    let text = toml::to_string_pretty(&waivers)
        .map_err(|error| format!("cannot render waivers {}: {error}", path.display()))?;
    fs::write(&path, text)
        .map_err(|error| format!("cannot write waivers {}: {error}", path.display()))?;
    Ok(WaiveOutcome {
        path,
        added: true,
        run,
    })
}

fn validate_date(value: &str) -> Result<(), String> {
    let bytes = value.as_bytes();
    let shape_ok = bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| index == 4 || index == 7 || byte.is_ascii_digit());
    if !shape_ok {
        return Err(format!("invalid --expires {value:?}; use YYYY-MM-DD"));
    }
    let month = value[5..7].parse::<u32>().unwrap_or(0);
    let day = value[8..10].parse::<u32>().unwrap_or(0);
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return Err(format!("invalid --expires {value:?}; use YYYY-MM-DD"));
    }
    Ok(())
}

fn waivers_path(project: &Path) -> Result<PathBuf, String> {
    Ok(normalize_project(project)?
        .join(WAIVERS_DIR)
        .join(WAIVERS_FILE))
}

pub(crate) fn today_utc() -> String {
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
