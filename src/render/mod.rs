use std::fs;
use std::path::PathBuf;

use crate::core::closeout::{CloseoutArgs, CloseoutError, DocImpact, DocImpactSignal};
use crate::core::{ensure_artifact_dir, run_dir, DocumentLane, Manifest, RouteArgs};

mod pack;

#[derive(Debug)]
pub(crate) struct PacketOutcome {
    pub(crate) packet_path: PathBuf,
    pub(crate) subagent_prompt_path: PathBuf,
    pub(crate) manifest_path: PathBuf,
    pub(crate) pack_path: Option<PathBuf>,
    pub(crate) warnings: Vec<String>,
}

pub(crate) fn write_route_packet(args: RouteArgs) -> Result<PacketOutcome, String> {
    let manifest = args.build_manifest()?;
    write_packet_files(manifest, None)
}

pub(crate) fn write_closeout_packet(args: CloseoutArgs) -> Result<PacketOutcome, CloseoutError> {
    let pack_options = args.pack.then_some(args.max_lines);
    let manifest = args.build_manifest()?;
    write_packet_files(manifest, pack_options).map_err(CloseoutError::Other)
}

fn write_packet_files(
    manifest: Manifest,
    pack_options: Option<usize>,
) -> Result<PacketOutcome, String> {
    let project = PathBuf::from(&manifest.project);
    ensure_artifact_dir(&project)?;
    let out_dir = run_dir(&project);
    fs::create_dir_all(&out_dir).map_err(|error| {
        format!(
            "cannot create output directory {}: {error}",
            out_dir.display()
        )
    })?;

    let manifest_path = out_dir.join("manifest.json");
    let packet_path = out_dir.join("packet.md");
    let subagent_prompt_path = out_dir.join("subagent-prompt.md");
    let pack_path = pack_options.map(|_| out_dir.join("pack.md"));
    let warnings = manifest
        .input_warnings
        .iter()
        .map(|warning| warning.render())
        .collect();

    write_text(&manifest_path, &render_manifest(&manifest)?)?;
    write_text(
        &packet_path,
        &render_packet(&manifest, &subagent_prompt_path),
    )?;
    write_text(&subagent_prompt_path, &render_subagent_prompt(&manifest))?;
    if let Some((path, max_lines)) = pack_path.as_ref().zip(pack_options) {
        write_text(path, &pack::render_pack(&manifest, max_lines))?;
    }

    Ok(PacketOutcome {
        packet_path,
        subagent_prompt_path,
        manifest_path,
        pack_path,
        warnings,
    })
}

fn render_manifest(manifest: &Manifest) -> Result<String, String> {
    serde_json::to_string_pretty(manifest)
        .map(|json| format!("{json}\n"))
        .map_err(|error| format!("cannot render manifest json: {error}"))
}

fn render_packet(manifest: &Manifest, subagent_prompt_path: &std::path::Path) -> String {
    let mut out = String::new();
    out.push_str("# Doc Maintenance Packet\n\n");
    out.push_str("## Inputs\n\n");
    out.push_str(&format!("- Command: `{}`\n", manifest.command));
    out.push_str(&format!("- Project: `{}`\n", manifest.project));
    out.push_str(&format!(
        "- Subagent prompt: `{}`\n",
        subagent_prompt_path.display()
    ));
    out.push_str(&format!(
        "- Topics: {}\n",
        list_or_none(&manifest.inputs.topic)
    ));
    if !manifest.input_warnings.is_empty() {
        out.push_str("\n## Input Warnings\n\n");
        for warning in &manifest.input_warnings {
            out.push_str(&format!("- {}\n", warning.render()));
        }
    }
    out.push_str("\n## Hard Rules\n\n");
    for rule in &manifest.rules {
        out.push_str(&format!("- {rule}\n"));
    }
    out.push_str("\n## Candidate Documents\n");
    render_lane(&mut out, manifest, DocumentLane::CurrentDevDocs);
    render_lane(&mut out, manifest, DocumentLane::RecordDocs);
    render_lane(&mut out, manifest, DocumentLane::ArchivedRecords);
    if let Some(closeout) = &manifest.closeout {
        out.push_str("\n## Change Source\n\n");
        out.push_str(&format!(
            "- Source: `{}` ({})\n",
            closeout.source.kind, closeout.source.detail
        ));
        out.push_str(&format!(
            "- Changed files: {}\n",
            list_or_none(&closeout.changed_files)
        ));
        out.push_str(&format!(
            "- Changed categories: {}\n",
            list_or_none(&closeout.changed_categories)
        ));
        out.push_str(&format!(
            "- New tokens: {}\n",
            list_or_none(&closeout.new_tokens)
        ));
        let documented = closeout
            .new_tokens
            .iter()
            .filter(|token| !closeout.missing_tokens.contains(token))
            .cloned()
            .collect::<Vec<_>>();
        out.push_str(&format!(
            "  - already documented: {}\n",
            list_or_none(&documented)
        ));
        out.push_str(&format!(
            "  - not yet documented: {}\n",
            list_or_none(&closeout.missing_tokens)
        ));
        out.push_str(&format!(
            "- Removed tokens (stale signal): {}\n",
            list_or_none(&closeout.removed_tokens)
        ));
        out.push_str(&format!(
            "- Missing tokens: {}\n",
            list_or_none(&closeout.missing_tokens)
        ));
        out.push_str(&format!(
            "- Low confidence reference: {}\n",
            low_confidence_or_none(closeout)
        ));
        out.push_str(&format!(
            "- Missing targets: {}\n",
            missing_targets_or_none(closeout)
        ));
        out.push_str("\n## Possible Doc Impact\n\n");
        if closeout.possible_doc_impact.is_empty() {
            out.push_str("- none\n");
        } else {
            for line in grouped_impacts(&closeout.possible_doc_impact) {
                out.push_str(&format!("- {line}\n"));
            }
        }
    }
    out.push_str("\n## Next Action\n\n");
    out.push_str("Send `subagent-prompt.md` to a read-only subagent. The main agent should read only the specific path:line evidence returned by that subagent before editing docs.\n");
    out
}

fn render_subagent_prompt(manifest: &Manifest) -> String {
    let mut out = String::new();
    out.push_str("# Read-Only Documentation Review\n\n");
    out.push_str("You are a read-only documentation reviewer. Do not edit files. Read only the candidate paths listed below and return concise `path:line` evidence.\n\n");
    out.push_str("## Return Shape\n\n");
    out.push_str(
        "- `stale`: existing lines that conflict with the requested summary or change evidence.\n",
    );
    out.push_str("- `update`: existing lines that should be updated, with the token or reason.\n");
    out.push_str("- `missing`: information that should be added, with the best target path.\n\n");
    if let Some(closeout) = &manifest.closeout {
        out.push_str("## Change Evidence\n\n");
        out.push_str(&format!(
            "- Changed files: {}\n",
            list_or_none(&closeout.changed_files)
        ));
        render_token_evidence(&mut out, "New tokens", &closeout.new_token_details);
        render_token_evidence(&mut out, "Removed tokens", &closeout.removed_token_details);
        render_token_evidence(
            &mut out,
            "Low confidence reference",
            &closeout.low_confidence_tokens,
        );
        out.push_str("- Possible doc impact:\n");
        if closeout.possible_doc_impact.is_empty() {
            out.push_str("  - none\n");
        } else {
            for line in grouped_impacts(&closeout.possible_doc_impact) {
                out.push_str(&format!("  - {line}\n"));
            }
        }
        out.push('\n');
    }
    out.push_str("## Candidate Paths\n");
    render_lane(&mut out, manifest, DocumentLane::CurrentDevDocs);
    render_lane(&mut out, manifest, DocumentLane::RecordDocs);
    render_lane(&mut out, manifest, DocumentLane::ArchivedRecords);
    out
}

fn render_token_evidence(
    out: &mut String,
    title: &str,
    tokens: &[crate::core::closeout::TokenObservation],
) {
    out.push_str(&format!("- {title}:\n"));
    if tokens.is_empty() {
        out.push_str("  - none\n");
        return;
    }
    for token in tokens {
        out.push_str(&format!(
            "  - `{}` ({}, {}, {}, source `{}`)\n",
            token.token,
            token.category.as_str(),
            token.confidence.as_str(),
            token.evidence,
            token.source_path
        ));
    }
}

/// One line per (signal, token, document): repeated hits in the same file
/// collapse into a line list so large docs stay readable. Stale hits in record
/// docs are labelled advisory because verify does not block on them.
fn grouped_impacts(impacts: &[DocImpact]) -> Vec<String> {
    let mut groups: Vec<(String, &str, &str, &DocumentLane, Vec<usize>)> = Vec::new();
    for impact in impacts {
        let signal = match (&impact.signal, &impact.lane) {
            (DocImpactSignal::Stale, DocumentLane::RecordDocs) => "stale (advisory)",
            (DocImpactSignal::Stale, _) => "stale",
            (DocImpactSignal::Update, _) => "update",
        };
        match groups.iter_mut().find(|(existing, token, path, lane, _)| {
            existing == signal
                && *token == impact.token
                && *path == impact.path
                && **lane == impact.lane
        }) {
            Some(group) => group.4.push(impact.line),
            None => groups.push((
                signal.to_string(),
                &impact.token,
                &impact.path,
                &impact.lane,
                vec![impact.line],
            )),
        }
    }
    groups
        .into_iter()
        .map(|(signal, token, path, lane, lines)| {
            let location = if lines.len() == 1 {
                format!("`{path}:{}`", lines[0])
            } else {
                format!(
                    "`{path}` lines {}",
                    lines
                        .iter()
                        .map(usize::to_string)
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            };
            format!("`{signal}` `{token}` at {location} ({})", lane.title())
        })
        .collect()
}

fn render_lane(out: &mut String, manifest: &Manifest, lane: DocumentLane) {
    out.push_str(&format!("\n### {}\n\n", lane.title()));
    let mut any = false;
    for candidate in manifest
        .candidates
        .iter()
        .filter(|candidate| candidate.lane == lane)
    {
        any = true;
        out.push_str(&format!(
            "- `{}` — {}{}\n",
            candidate.path,
            candidate.reason,
            if candidate.archived {
                "; archived: list only, do not read or edit"
            } else {
                ""
            }
        ));
    }
    if !any {
        out.push_str("- none\n");
    }
}

fn list_or_none(values: &[String]) -> String {
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

fn missing_targets_or_none(closeout: &crate::core::closeout::CloseoutManifest) -> String {
    if closeout.missing_targets.is_empty() {
        return "none".to_string();
    }
    closeout
        .missing_targets
        .iter()
        .map(|target| {
            format!(
                "`{}` -> `{}` ({})",
                target.token,
                target.path,
                target.lane.title()
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn low_confidence_or_none(closeout: &crate::core::closeout::CloseoutManifest) -> String {
    if closeout.low_confidence_tokens.is_empty() {
        return "none".to_string();
    }
    closeout
        .low_confidence_tokens
        .iter()
        .map(|token| {
            format!(
                "`{}` ({}, {}, {})",
                token.token,
                token.category.as_str(),
                token.confidence.as_str(),
                token.evidence
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn write_text(path: &std::path::Path, text: &str) -> Result<(), String> {
    fs::write(path, text).map_err(|error| format!("cannot write {}: {error}", path.display()))
}
