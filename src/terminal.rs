use std::io::{self, IsTerminal};

use crate::Cli;

#[derive(Clone, Copy, Debug)]
pub(crate) struct OutputMode {
    banner: bool,
    color: bool,
}

impl OutputMode {
    pub(crate) fn from_cli(cli: &Cli) -> Self {
        let interactive = io::stdout().is_terminal();
        let no_color = std::env::var_os("NO_COLOR").is_some();
        let plain = cli.plain || no_color || !interactive;

        Self {
            banner: !plain && !cli.no_banner,
            color: !plain,
        }
    }

    pub(crate) fn status(self, kind: StatusKind, text: &str) {
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

    pub(crate) fn banner(self) {
        if self.banner {
            println!("⚙ Yan Maintenance");
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum StatusKind {
    Ok,
    Warn,
}
