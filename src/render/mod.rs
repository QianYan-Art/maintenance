use std::fs;
use std::path::PathBuf;

use crate::core::closeout::{CloseoutArgs, CloseoutError, DocImpactSignal};
use crate::core::{run_dir, DocumentLane, Manifest, RouteArgs};

mod pack;

#[derive(Debug)]
pub(crate) struct PacketOutcome {
    pub(crate) packet_path: PathBuf,
    pub(crate) subagent_prompt_path: PathBuf,
    pub(crate) manifest_path: PathBuf,
    pub(crate) pack_path: Option<PathBuf>,
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
    let out_dir = run_dir(PathBuf::from(&manifest.project).as_path());
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
    out.push_str("\n## Hard Rules\n\n");
    for rule in &manifest.rules {
        out.push_str(&format!("- {rule}\n"));
    }
    out.push_str("\n## Candidate Documents\n");
    render_lane(&mut out, manifest, DocumentLane::CurrentDevDocs);
    render_lane(&mut out, manifest, DocumentLane::KBaseRecords);
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
        out.push_str(&format!(
            "- Removed tokens (stale signal): {}\n",
            list_or_none(&closeout.removed_tokens)
        ));
        out.push_str(&format!(
            "- Missing tokens: {}\n",
            list_or_none(&closeout.missing_tokens)
        ));
        out.push_str("\n## Possible Doc Impact\n\n");
        if closeout.possible_doc_impact.is_empty() {
            out.push_str("- none\n");
        } else {
            for impact in &closeout.possible_doc_impact {
                let signal = match impact.signal {
                    DocImpactSignal::Stale => "stale",
                    DocImpactSignal::Update => "update",
                };
                out.push_str(&format!(
                    "- `{}` `{}` at `{}:{}` ({})\n",
                    signal,
                    impact.token,
                    impact.path,
                    impact.line,
                    impact.lane.title()
                ));
            }
        }
    }
    out.push_str("\n## Next Action\n\n");
    out.push_str("Send `subagent-prompt.md` to a read-only subagent. The main Codex should read only the specific path:line evidence returned by that subagent before editing docs.\n");
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
    out.push_str("## Candidate Paths\n");
    render_lane(&mut out, manifest, DocumentLane::CurrentDevDocs);
    render_lane(&mut out, manifest, DocumentLane::KBaseRecords);
    render_lane(&mut out, manifest, DocumentLane::ArchivedRecords);
    out
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

fn write_text(path: &std::path::Path, text: &str) -> Result<(), String> {
    fs::write(path, text).map_err(|error| format!("cannot write {}: {error}", path.display()))
}
