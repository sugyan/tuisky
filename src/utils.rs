use crate::tui;
use color_eyre::config::HookBuilder;
use std::panic;

pub fn initialize_panic_handler() -> color_eyre::Result<()> {
    let (panic_hook, eyre_hook) = HookBuilder::default().into_hooks();
    eyre_hook.install()?;

    let panic_hook = panic_hook.into_panic_hook();
    panic::set_hook(Box::new(move |panic_info| {
        tui::restore().expect("failed to restore terminal");
        panic_hook(panic_info);
    }));
    Ok(())
}
