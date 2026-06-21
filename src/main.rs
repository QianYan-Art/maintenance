use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};

mod core;
mod render;
mod terminal;

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
            if args.git.is_none() && args.since.is_none() && args.change_manifest.is_none() {
                println!("needs_input: changed_source");
                return ExitCode::from(2);
            }

            output.status(
                StatusKind::Ok,
                &format!(
                    "closeout accepted for project {}",
                    args.docs.project.display()
                ),
            );
            output.status(
                StatusKind::Warn,
                "change-source analysis will be filled by the closeout slice",
            );
            ExitCode::SUCCESS
        }
        Command::Verify(args) => {
            output.status(
                StatusKind::Warn,
                &format!("verify accepted for project {}", args.project.display()),
            );
            output.status(
                StatusKind::Warn,
                "verification rules will be filled by the closeout slice",
            );
            ExitCode::SUCCESS
        }
    }
}
