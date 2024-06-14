use color_eyre::eyre::Result;
use crossterm::execute;
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use crossterm::terminal::{EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::backend::Backend;
use ratatui::Terminal;
use std::io::{stdout, Write};
use std::ops::{Deref, DerefMut};

pub fn io() -> impl Write {
    stdout()
}

pub struct Tui<B>
where
    B: Backend,
{
    terminal: Terminal<B>,
}

impl<B> Tui<B>
where
    B: Backend,
{
    /// Constructs a new instance of [`Tui`].
    pub fn new(terminal: Terminal<B>) -> Self {
        Self { terminal }
    }
}

impl<B> Deref for Tui<B>
where
    B: Backend,
{
    type Target = Terminal<B>;

    fn deref(&self) -> &Self::Target {
        &self.terminal
    }
}

impl<B> DerefMut for Tui<B>
where
    B: Backend,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.terminal
    }
}

/// Initialize the terminal
pub fn init() -> Result<()> {
    execute!(io(), EnterAlternateScreen)?;
    enable_raw_mode()?;
    Ok(())
}

/// Restore the terminal to its original state
pub fn restore() -> Result<()> {
    execute!(io(), LeaveAlternateScreen)?;
    disable_raw_mode()?;
    Ok(())
}
