use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use crate::core::verify::VerifyOutcome;
use crate::core::{normalize_project, Manifest};

#[derive(Debug)]
pub(crate) struct RunReport {
    pub(crate) run_id: String,
    pub(crate) source: String,
    pub(crate) changed_files: usize,
    pub(crate) high_tokens: usize,
    pub(crate) low_tokens: usize,
    pub(crate) waived_tokens: usize,
    pub(crate) missing_tokens: usize,
    pub(crate) input_warnings: usize,
    pub(crate) stale_advisory: usize,
    pub(crate) verify_result: String,
}

pub(crate) fn report_project(project: &Path, last: usize) -> Result<Vec<RunReport>, String> {
    let runs = normalize_project(project)?
        .join(".doc-maintenance")
        .join("runs");
    let mut run_dirs = fs::read_dir(&runs)
        .map_err(|error| format!("cannot read runs directory {}: {error}", runs.display()))?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.is_dir())
        .collect::<Vec<_>>();
    run_dirs.sort();
    run_dirs.reverse();
    run_dirs
        .into_iter()
        .take(last)
        .map(|run_dir| summarize_run(&run_dir))
        .collect()
}

fn summarize_run(run_dir: &Path) -> Result<RunReport, String> {
    let run_id = run_dir
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("<unknown>")
        .to_string();
    let manifest_path = run_dir.join("manifest.json");
    if !manifest_path.exists() {
        return Ok(RunReport {
            run_id,
            source: "incomplete".to_string(),
            changed_files: 0,
            high_tokens: 0,
            low_tokens: 0,
            waived_tokens: 0,
            missing_tokens: 0,
            input_warnings: 0,
            stale_advisory: 0,
            verify_result: "incomplete".to_string(),
        });
    }
    let manifest_text = fs::read_to_string(&manifest_path)
        .map_err(|error| format!("cannot read {}: {error}", manifest_path.display()))?;
    let manifest: Manifest = serde_json::from_str(&manifest_text)
        .map_err(|error| format!("invalid manifest {}: {error}", manifest_path.display()))?;
    let input_warnings = manifest.input_warnings.len();
    let (verify_result, stale_advisory) = verify_result(run_dir)?;
    let Some(closeout) = manifest.closeout else {
        return Ok(RunReport {
            run_id,
            source: manifest.command,
            changed_files: 0,
            high_tokens: 0,
            low_tokens: 0,
            waived_tokens: 0,
            missing_tokens: 0,
            input_warnings,
            stale_advisory,
            verify_result,
        });
    };
    let high_tokens = closeout
        .new_tokens
        .iter()
        .chain(closeout.removed_tokens.iter())
        .cloned()
        .collect::<BTreeSet<_>>()
        .len();
    Ok(RunReport {
        run_id,
        source: format!("{}:{}", closeout.source.kind, closeout.source.detail),
        changed_files: closeout.changed_files.len(),
        high_tokens,
        low_tokens: closeout.low_confidence_tokens.len(),
        waived_tokens: closeout.ignored_tokens.len(),
        missing_tokens: closeout.missing_tokens.len(),
        input_warnings,
        stale_advisory,
        verify_result,
    })
}

fn verify_result(run_dir: &Path) -> Result<(String, usize), String> {
    let path = run_dir.join("outcome.json");
    if !path.exists() {
        return Ok(("unverified".to_string(), 0));
    }
    let text = fs::read_to_string(&path)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    let outcome: VerifyOutcome = serde_json::from_str(&text)
        .map_err(|error| format!("invalid {}: {error}", path.display()))?;
    Ok((outcome.result, outcome.stale_advisory.len()))
}
