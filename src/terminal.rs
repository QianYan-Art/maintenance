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
        Self::from_parts(cli.plain, cli.no_banner, interactive, no_color)
    }

    fn from_parts(plain_flag: bool, no_banner: bool, interactive: bool, no_color: bool) -> Self {
        let plain = plain_flag || no_color || !interactive;

        Self {
            banner: !plain && !no_banner,
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

#[cfg(test)]
mod tests {
    use super::OutputMode;

    #[test]
    fn tty_defaults_to_banner_and_color() {
        let mode = OutputMode::from_parts(false, false, true, false);

        assert!(mode.banner);
        assert!(mode.color);
    }

    #[test]
    fn plain_no_color_and_non_tty_disable_banner() {
        assert!(!OutputMode::from_parts(true, false, true, false).banner);
        assert!(!OutputMode::from_parts(false, false, true, true).banner);
        assert!(!OutputMode::from_parts(false, false, false, false).banner);
        assert!(!OutputMode::from_parts(false, true, true, false).banner);
    }
}
