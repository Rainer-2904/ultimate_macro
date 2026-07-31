mod models;
mod api;
mod db;
mod calc;
mod logic;

use log::{info, error, LevelFilter};
use simplelog::*;
use std::fs::File;
use chrono::Local;

#[tokio::main]
async fn main() {
    // 1. Initialize the logger to write to both terminal and a .log file
    CombinedLogger::init(
        vec![
            TermLogger::new(LevelFilter::Info, Config::default(), TerminalMode::Mixed, ColorChoice::Auto),
            WriteLogger::new(LevelFilter::Info, Config::default(), File::create("execution.log").unwrap()),
        ]
    ).unwrap();

    info!("[S] Logger initialized");
    info!("[S] Starting Master Integration Test...");

    // 2. Test DB Initialization
    if let Err(e) = db::init_db() {
        error!("[ERROR] DB init failed: {}", e);
        return;
    }
    info!("[S] DB tables created");

    // 3. Test Daily Goals (Insert & Retrieve)
    let my_goals = models::DailyMacroGoal {
        target_kcal: 2500.0,
        target_proteins: 160.0,
        target_carbohydrates: 300.0,
        target_fat: 70.0,
    };

    if let Err(e) = db::set_daily_goals(&my_goals) {
        error!("[ERROR] Failed to set daily goals: {}", e);
    } else {
        info!("[S] Daily goals set");
    }

    match db::get_daily_goals() {
        Ok(Some(_)) => info!("[S] Daily goals retrieved"),
        Ok(None) => error!("[ERROR] Daily goals not found after insert"),
        Err(e) => error!("[ERROR] Failed to get daily goals: {}", e),
    }

    // 4. Test API Fetch & Product Insertion
    let test_barcode = "5941355009346";
    let mut api_food_id = 0;

    match api::fetch_food_by_barcode(test_barcode).await {
        Ok(food) => {
            info!("[S] Fetched food from API");
            match db::insert_food(&food) {
                Ok(id) => {
                    info!("[S] Food item inserted");
                    api_food_id = id;
                }
                Err(e) => error!("[ERROR] Failed to insert API food: {}", e),
            }
        }
        Err(e) => error!("[ERROR] API fetch failed: {}", e),
    }

    // 5. Test Custom Food (Manual Entry)
    let mut custom_food_id = 0;
    match db::insert_custom_food("Test Apple", 52.0, 0.3, 13.8, 0.2, 100.0) {
        Ok(id) => {
            info!("[S] Custom food inserted");
            custom_food_id = id;
        }
        Err(e) => error!("[ERROR] Failed to insert custom food: {}", e),
    }

    // 6. Test Logging Consumption
    // Note: Assuming log_food_consumption signature is (food_id, quantity_in_grams) based on last update
    let mut log_id_to_modify = 0;
    
    if api_food_id != 0 {
        match db::log_food_consumption(api_food_id, 250.0) {
            Ok(log_id) => {
                info!("[S] Food consumption logged");
                log_id_to_modify = log_id;
            }
            Err(e) => error!("[ERROR] Failed to log API food: {}", e),
        }
    }

    if custom_food_id != 0 {
        match db::log_food_consumption(custom_food_id, 150.0) {
            Ok(_) => info!("[S] Custom food consumption logged"),
            Err(e) => error!("[ERROR] Failed to log custom food: {}", e),
        }
    }

    // 7. Test Updating Log Quantity
    if log_id_to_modify != 0 {
        match db::update_log_quantity(log_id_to_modify, 500.0) {
            Ok(rows) if rows > 0 => info!("[S] Log quantity updated"),
            Ok(_) => error!("[ERROR] Log ID not found for update"),
            Err(e) => error!("[ERROR] Failed to update log: {}", e),
        }
    }

    // 8. Test Retrieving Detailed Daily Log
    let today = Local::now().format("%Y-%m-%d").to_string();
    match db::get_logged_foods_for_date(&today) {
        Ok(items) => {
            info!("[S] Daily log list retrieved ({} items)", items.len());
        }
        Err(e) => error!("[ERROR] Failed to retrieve daily list: {}", e),
    }

    // 9. Test Deleting a Log
    if log_id_to_modify != 0 {
        match db::delete_log_entry(log_id_to_modify) {
            Ok(rows) if rows > 0 => info!("[S] Log entry deleted"),
            Ok(_) => error!("[ERROR] Log ID not found for deletion"),
            Err(e) => error!("[ERROR] Failed to delete log: {}", e),
        }
    }

    info!("[S] Master test completed successfully");
}