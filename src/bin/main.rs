use color_eyre::Result;
use tuisky::app::App;
use tuisky::{errors, tui};

fn main() -> Result<()> {
    errors::install_hooks()?;
    let mut terminal = tui::init()?;
    App::default().run(&mut terminal)?;
    tui::restore()?;
    Ok(())
}
