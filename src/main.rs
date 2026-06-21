use std::io::{self, IsTerminal};
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};

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
    topic: Option<String>,
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

#[derive(Clone, Copy, Debug)]
struct OutputMode {
    banner: bool,
    color: bool,
}

impl OutputMode {
    fn from_cli(cli: &Cli) -> Self {
        let interactive = io::stdout().is_terminal();
        let no_color = std::env::var_os("NO_COLOR").is_some();
        let plain = cli.plain || no_color || !interactive;

        Self {
            banner: !plain && !cli.no_banner,
            color: !plain,
        }
    }

    fn status(self, kind: StatusKind, text: &str) {
        let symbol = match kind {
            StatusKind::Ok => "✓",
            StatusKind::Warn => "⚠",
        };
        let rendered = if self.color {
            let code = match kind {
                StatusKind::Ok => "32",
                StatusKind::Warn => "33",
            };
            format!("\x1b[{code}m{symbol}\x1b[0m {text}")
        } else {
            format!("{symbol} {text}")
        };
        println!("│ {rendered}");
    }

    fn banner(self) {
        if self.banner {
            println!("⚙ Yan Maintenance");
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum StatusKind {
    Ok,
    Warn,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let output = OutputMode::from_cli(&cli);
    output.banner();

    match cli.command {
        Command::Route(args) => {
            output.status(
                StatusKind::Ok,
                &format!("route accepted for project {}", args.project.display()),
            );
            output.status(
                StatusKind::Warn,
                "packet generation will be filled by the route slice",
            );
            ExitCode::SUCCESS
        }
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
