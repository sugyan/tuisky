use std::io::Result;
use tuisky::app::App;
use tuisky::tui;

fn main() -> Result<()> {
    let mut terminal = tui::init()?;
    let app_result = App::default().run(&mut terminal);
    tui::restore()?;
    app_result
}
