use crate::tui;
use color_eyre::{config::HookBuilder, eyre, Result};
use directories::ProjectDirs;
use std::{panic, path::PathBuf, process};

pub fn initialize_panic_handler() -> Result<()> {
    let (panic_hook, eyre_hook) = HookBuilder::default().into_hooks();
    eyre_hook.install()?;

    let panic_hook = panic_hook.into_panic_hook();
    panic::set_hook(Box::new(move |panic_info| {
        tui::restore().expect("failed to restore terminal");
        panic_hook(panic_info);
        process::exit(1);
    }));
    Ok(())
}

pub fn get_data_dir() -> Result<PathBuf> {
    Ok(project_dirs()?.data_dir().to_path_buf())
}

pub fn get_config_dir() -> Result<PathBuf> {
    let home = std::env::var("HOME")
        .map(PathBuf::from)
        .map_err(|_| eyre::eyre!("failed to get HOME env"))?;
    Ok(home.join(".config").join("tuisky"))
}

fn project_dirs() -> Result<ProjectDirs> {
    ProjectDirs::from("com", "sugyan", "tuisky")
        .ok_or_else(|| eyre::eyre!("failed to get project directories"))
}
