mod models;
mod api;
mod db;
mod calc;
mod logic;

use log::{info, error, LevelFilter};
use simplelog::*;
use std::fs::File;
use chrono::Local;

slint::include_modules!();

#[tokio::main]
// Avoided using 'Result' on main to properly log errors and exit in a civilized manner.
// Otherwise, the error would be thrown to tokio instead
// and the program would exit with a non-zero code without any logging.
// WHICH IS NOT IDEAL.
async fn main() {
    if let Err(e) = db::init_db() {
        error!("Failed to initialize database: {}", e);
        return;
    }

    let ui = match MainWindow::new() {
        Ok(ui) => ui,
        Err(e) => {
            error!("Failed to create UI: {}", e);
            return;
        }
    };

    if let Err(e) = ui.run() {
        error!("Failed to run UI: {}", e);
    }
}