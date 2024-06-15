use color_eyre::Result;
use tuisky::app::App;
use tuisky::utils::initialize_panic_handler;

#[tokio::main]
async fn main() -> Result<()> {
    if let Err(e) = tui_logger::init_logger(log::LevelFilter::Debug) {
        panic!("failed to initialize logger: {e}");
    }
    tui_logger::set_default_level(log::LevelFilter::Debug);

    initialize_panic_handler()?;

    let mut app = App::new(10.0);
    app.run().await?;

    Ok(())
}
