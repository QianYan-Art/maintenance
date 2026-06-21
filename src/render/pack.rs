use std::fs;
use std::path::PathBuf;

use crate::core::closeout::DocImpactSignal;
use crate::core::Manifest;

pub(crate) fn render_pack(manifest: &Manifest, max_lines: usize) -> String {
    let limit = max_lines.max(1);
    let mut lines = Vec::new();
    push(&mut lines, "# Doc Maintenance Fallback Pack");
    push(&mut lines, "");
    push(
        &mut lines,
        "This pack is a bounded fallback for when a read-only subagent is unavailable.",
    );
    push(&mut lines, "It is not a long-term fact source.");
    push(&mut lines, "");
    push(&mut lines, "## Candidate Paths");
    for candidate in &manifest.candidates {
        push(
            &mut lines,
            &format!(
                "- {} [{}] — {}{}",
                candidate.path,
                candidate.lane.title(),
                candidate.reason,
                if candidate.archived {
                    "; archived: list only"
                } else {
                    ""
                }
            ),
        );
    }
    if let Some(closeout) = &manifest.closeout {
        push(&mut lines, "");
        push(&mut lines, "## Tokens");
        push(
            &mut lines,
            &format!("- New: {}", join_or_none(&closeout.new_tokens)),
        );
        push(
            &mut lines,
            &format!("- Removed: {}", join_or_none(&closeout.removed_tokens)),
        );
        push(
            &mut lines,
            &format!("- Missing: {}", join_or_none(&closeout.missing_tokens)),
        );
        push(&mut lines, "");
        push(&mut lines, "## Hit Lines");
        for impact in &closeout.possible_doc_impact {
            let signal = match impact.signal {
                DocImpactSignal::Stale => "stale",
                DocImpactSignal::Update => "update",
            };
            push(
                &mut lines,
                &format!(
                    "- {signal} `{}` at {}:{}",
                    impact.token, impact.path, impact.line
                ),
            );
            for snippet in snippet_lines(manifest, &impact.path, impact.line) {
                push(&mut lines, &format!("  {snippet}"));
            }
        }
    }

    if lines.len() > limit {
        lines.truncate(limit.saturating_sub(1));
        lines.push("... truncated by --max-lines".to_string());
    }
    format!("{}\n", lines.join("\n"))
}

fn snippet_lines(manifest: &Manifest, relative_path: &str, line: usize) -> Vec<String> {
    let path = PathBuf::from(&manifest.project).join(relative_path);
    let Ok(text) = fs::read_to_string(path) else {
        return Vec::new();
    };
    let all = text.lines().collect::<Vec<_>>();
    if all.is_empty() {
        return Vec::new();
    }
    let index = line.saturating_sub(1).min(all.len() - 1);
    let mut output = Vec::new();
    if let Some(heading) = nearest_heading(&all, index) {
        output.push(format!("title: {heading}"));
    }
    if index > 0 {
        output.push(format!("{}: {}", index, all[index - 1]));
    }
    output.push(format!("{}: {}", index + 1, all[index]));
    if index + 1 < all.len() {
        output.push(format!("{}: {}", index + 2, all[index + 1]));
    }
    output
}

fn nearest_heading(lines: &[&str], index: usize) -> Option<String> {
    lines
        .iter()
        .take(index + 1)
        .rev()
        .find(|line| line.trim_start().starts_with('#'))
        .map(|line| line.trim().to_string())
}

fn push(lines: &mut Vec<String>, line: &str) {
    lines.push(line.to_string());
}

fn join_or_none(values: &[String]) -> String {
    if values.is_empty() {
        "none".to_string()
    } else {
        values
            .iter()
            .map(|value| format!("`{value}`"))
            .collect::<Vec<_>>()
            .join(", ")
    }
}
