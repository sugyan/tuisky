use clap::Parser;
use color_eyre::Result;
use tuisky::app::App;
use tuisky::utils::initialize_panic_handler;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
/// TUI Client for Bluesky.
struct Args {
    /// Maximum number of columns to display.
    /// The number of columns will be determined by the terminal width.
    #[arg(short, long)]
    columns: Option<usize>,
    /// Development mode
    #[arg(short, long)]
    dev: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    if let Err(e) = tui_logger::init_logger(log::LevelFilter::Debug) {
        panic!("failed to initialize logger: {e}");
    }
    tui_logger::set_default_level(log::LevelFilter::Debug);

    initialize_panic_handler()?;

    let mut app = App::new(args.columns, args.dev);
    app.run().await?;

    Ok(())
}
