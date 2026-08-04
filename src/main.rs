mod models;
mod api;
mod db;
mod calc;
mod logic;

use log::{info, error};
use slint::SharedString;

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

    // Initial settings
    ui.set_total_calories(1850);
    let ui_handle = ui.as_weak();

    // Callback handle
    ui.on_open_scanner(move || {
        info!("Initializing scanner...");
        let ui_clone = ui_handle.clone();

        tokio::spawn(async move {
            let test_barcode = "5449000000996";
            let test_quantity = 330.0;

            let _ = ui_clone.upgrade_in_event_loop(|ui_instance| {
                ui_instance.set_scanner_status_text(SharedString::from("Searching in local cache..."));
            });

            // We first check local cache
            match db::get_product_by_barcode(test_barcode) {
                Ok(Some(food)) => {
                    info!("Product '{}' found in local DB cache!", food.product_name);

                    if let Err(e) = db::log_food_consumption(food.id.unwrap(), test_quantity) {
                        error!("Failed to log consumption: {}", e);
                    }

                    let msg = format!("Logged: {}", food.product_name);
                    let _ = ui_clone.upgrade_in_event_loop(move |ui_instance| {
                        ui_instance.set_scanner_status_text(SharedString::from(msg));
                    });
                }
                // If local cache does not have the product saved we fetch it from api
                Ok(None) => {
                    info!("Barcode {} not found in cache, fethcing from OpenFoodFacts...", test_barcode);

                    let _ = ui_clone.upgrade_in_event_loop(|ui_instance| {
                        ui_instance.set_scanner_status_text(SharedString::from("Fetching..."));
                    });

                    match api::fetch_food_by_barcode(test_barcode).await {
                        Ok(new_food) => {
                            // Save it for later inserts
                            match db::insert_food(&new_food) {
                                Ok(new_id) => {
                                    let _ = db::log_food_consumption(new_id, test_quantity);
                                    let msg = format!("Saved and logged: {}", new_food.product_name);

                                    let _ = ui_clone.upgrade_in_event_loop(move |ui_instance| {
                                        ui_instance.set_scanner_status_text(SharedString::from(msg));
                                    });
                                }
                                Err(e) => error!("Failed to insert food to local DB: {}", e),
                            }
                        }
                        Err(e) => {
                            error!("API fetch failed: {}", e);
                            let _ = ui_clone.upgrade_in_event_loop(|ui_instance| {
                                ui_instance.set_scanner_status_text(SharedString::from("Product not found / Offline"));
                            });
                        }
                    }
                }
                Err(e) => error!("DB query error: {}", e),
            }
        });
    });

    if let Err(e) = ui.run() {
        error!("Failed to run UI: {}", e);
    }
}