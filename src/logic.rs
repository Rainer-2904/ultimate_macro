use crate::{api, db};
use simplelog::{Config, LevelFilter, WriteLogger};
use std::fs::OpenOptions;

// Lookup/cache is deliberately separate from consumption. A scan must never
// record a meal until the user confirms the quantity.
pub async fn lookup_food(barcode: &str) -> Result<crate::models::FoodItem, String> {
    if let Some(food) = db::get_product_by_barcode(barcode).map_err(|e| e.to_string())? {
        return Ok(food);
    }
    let mut food = api::fetch_food_by_barcode(barcode).await?;
    food.id = Some(db::insert_food(&food).map_err(|e| e.to_string())?);
    Ok(food)
}
// Initialize logging to file (app.log)
pub fn init_logger() {
    let log_file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true) // Rewrite the file on each launch
        .open(crate::storage::path("app.log"))
        .expect("Failed to open app.log for writing");

    let _ = WriteLogger::init(LevelFilter::Info, Config::default(), log_file);
}
