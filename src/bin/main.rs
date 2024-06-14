use color_eyre::Result;
use tuisky::app::App;
use tuisky::utils::initialize_panic_handler;

#[tokio::main]
async fn main() -> Result<()> {
    initialize_panic_handler()?;

    let mut app = App::new();
    app.run().await?;

    Ok(())
}
