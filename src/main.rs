use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};

mod core;
mod render;
mod terminal;

use core::closeout::{CloseoutArgs as CoreCloseoutArgs, CloseoutError};
use core::diff::ChangeSourceRequest;
use core::RouteArgs;
use terminal::{OutputMode, StatusKind};

#[derive(Debug, Parser)]
#[command(name = "maintenance")]
#[command(about = "Generate Codex-readable document maintenance packets")]
#[command(subcommand_required = true)]
struct Cli {
    #[arg(long, global = true, help = "Disable banner and ANSI color")]
    plain: bool,

    #[arg(long, global = true, help = "Disable the human banner")]
    no_banner: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    #[command(about = "Create a document reading route packet")]
    Route(CommonDocArgs),

    #[command(about = "Create a closeout packet from a content-bearing change source")]
    Closeout(CloseoutArgs),

    #[command(about = "Verify that document closeout expectations are satisfied")]
    Verify(VerifyArgs),
}

#[derive(Debug, Parser)]
struct CommonDocArgs {
    #[arg(long, default_value = ".")]
    project: PathBuf,

    #[arg(long = "dev-docs")]
    dev_docs: Vec<PathBuf>,

    #[arg(long = "record-docs")]
    record_docs: Vec<PathBuf>,

    #[arg(long = "summary-source")]
    summary_source: Vec<PathBuf>,

    #[arg(long)]
    topic: Vec<String>,
}

#[derive(Debug, Parser)]
struct CloseoutArgs {
    #[command(flatten)]
    docs: CommonDocArgs,

    #[arg(long)]
    git: Option<String>,

    #[arg(long)]
    since: Option<String>,

    #[arg(long = "change-manifest")]
    change_manifest: Option<PathBuf>,

    #[arg(long)]
    pack: bool,

    #[arg(long = "max-lines", default_value_t = 200)]
    max_lines: usize,
}

#[derive(Debug, Parser)]
struct VerifyArgs {
    #[arg(long, default_value = ".")]
    project: PathBuf,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let output = OutputMode::from_cli(&cli);
    output.banner();

    match cli.command {
        Command::Route(args) => match render::write_route_packet(RouteArgs {
            project: args.project,
            dev_docs: args.dev_docs,
            record_docs: args.record_docs,
            summary_source: args.summary_source,
            topic: args.topic,
        }) {
            Ok(outcome) => {
                output.status(
                    StatusKind::Ok,
                    &format!("packet: {}", outcome.packet_path.display()),
                );
                output.status(
                    StatusKind::Ok,
                    &format!(
                        "subagent prompt: {}",
                        outcome.subagent_prompt_path.display()
                    ),
                );
                output.status(
                    StatusKind::Ok,
                    &format!("manifest: {}", outcome.manifest_path.display()),
                );
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("error: {error}");
                ExitCode::from(1)
            }
        },
        Command::Closeout(args) => {
            let source = change_source(&args);
            if source.is_none() {
                output.status(StatusKind::Warn, "needs_input: changed_source");
                return ExitCode::from(2);
            }

            match render::write_closeout_packet(CoreCloseoutArgs {
                route: RouteArgs {
                    project: args.docs.project,
                    dev_docs: args.docs.dev_docs,
                    record_docs: args.docs.record_docs,
                    summary_source: args.docs.summary_source,
                    topic: args.docs.topic,
                },
                source,
                pack: args.pack,
                max_lines: args.max_lines,
            }) {
                Ok(outcome) => {
                    output.status(
                        StatusKind::Ok,
                        &format!("packet: {}", outcome.packet_path.display()),
                    );
                    output.status(
                        StatusKind::Ok,
                        &format!(
                            "subagent prompt: {}",
                            outcome.subagent_prompt_path.display()
                        ),
                    );
                    output.status(
                        StatusKind::Ok,
                        &format!("manifest: {}", outcome.manifest_path.display()),
                    );
                    if let Some(pack_path) = outcome.pack_path {
                        output.status(StatusKind::Ok, &format!("pack: {}", pack_path.display()));
                    }
                    ExitCode::SUCCESS
                }
                Err(CloseoutError::NeedsInput) => {
                    output.status(StatusKind::Warn, "needs_input: changed_source");
                    ExitCode::from(2)
                }
                Err(CloseoutError::Other(error)) => {
                    eprintln!("error: {error}");
                    ExitCode::from(1)
                }
            }
        }
        Command::Verify(args) => match core::verify::verify_project(&args.project) {
            Ok(report) if report.is_ok() => {
                output.status(StatusKind::Ok, "verify passed");
                ExitCode::SUCCESS
            }
            Ok(report) => {
                println!("verify_failed");
                for token in report.stale_remaining {
                    println!("stale_remaining: {token}");
                }
                for token in report.missing_remaining {
                    println!("missing_remaining: {token}");
                }
                ExitCode::from(2)
            }
            Err(error) => {
                eprintln!("error: {error}");
                ExitCode::from(1)
            }
        },
    }
}

fn change_source(args: &CloseoutArgs) -> Option<ChangeSourceRequest> {
    if let Some(manifest) = &args.change_manifest {
        return Some(ChangeSourceRequest::ChangeManifest(manifest.clone()));
    }
    if let Some(revision) = &args.since {
        return Some(ChangeSourceRequest::Since(revision.clone()));
    }
    match args.git.as_deref() {
        Some("uncommitted") => Some(ChangeSourceRequest::GitUncommitted),
        Some(_) | None => None,
    }
}
