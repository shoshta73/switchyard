use std::io::{self, Stdout, Write};

use anyhow::{Context, Result};

use crossterm::{
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};

#[derive(Default)]
pub(crate) struct Guard {
    raw_mode_enabled: bool,
    alternate_screen_enabled: bool,
}

impl Guard {
    pub(crate) fn enable_raw_mode(&mut self) -> io::Result<()> {
        enable_raw_mode()?;
        self.raw_mode_enabled = true;
        Ok(())
    }

    pub(crate) fn enter_alternate_screen<W: Write>(&mut self, writer: &mut W) -> io::Result<()> {
        execute!(writer, EnterAlternateScreen)?;
        self.alternate_screen_enabled = true;
        Ok(())
    }

    pub(crate) fn restore<W: Write>(&mut self, writer: &mut W) -> io::Result<()> {
        if self.alternate_screen_enabled {
            execute!(writer, LeaveAlternateScreen)?;
            self.alternate_screen_enabled = false;
        }

        if self.raw_mode_enabled {
            disable_raw_mode()?;
            self.raw_mode_enabled = false;
        }

        Ok(())
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        let _ = self.restore(&mut io::stdout());
    }
}

pub(crate) fn init() -> Result<(Terminal<CrosstermBackend<Stdout>>, Guard)> {
    let mut terminal_guard = Guard::default();
    terminal_guard
        .enable_raw_mode()
        .context("failed to enable raw mode")?;

    let mut stdout = io::stdout();
    terminal_guard
        .enter_alternate_screen(&mut stdout)
        .context("failed to enter alternate screen")?;

    let terminal =
        Terminal::new(CrosstermBackend::new(stdout)).context("failed to initialize terminal")?;
    Ok((terminal, terminal_guard))
}
