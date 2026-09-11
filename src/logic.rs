use crate::{api, db};
use simplelog::{Config, LevelFilter, WriteLogger};
use std::fs::OpenOptions;

// Lookup/cache is deliberately separate from consumption. A scan must never
// record a meal until the user confirms the quantity.
pub async fn lookup_food(barcode: &str) -> Result<crate::models::FoodItem, String> {
    if let Some(food) = db::get_product_by_barcode(barcode).map_err(|e| {
        log::error!("Could not read cached product {barcode}: {e}");
        "Could not read the local food cache.".to_string()
    })? {
        return Ok(food);
    }
    let mut food = api::fetch_food_by_barcode(barcode).await?;
    food.id = Some(db::insert_food(&food).map_err(|e| {
        log::error!("Could not cache product {barcode}: {e}");
        "Product found, but it could not be saved locally.".to_string()
    })?);
    Ok(food)
}
// Initialize logging to file (app.log)
pub fn init_logger() {
    let log_file = OpenOptions::new()
        .create(true)
        // Append so an Activity restart cannot truncate another live logger's
        // file and destroy the error that caused the restart.
        .append(true)
        .open(crate::storage::path("app.log"))
        .expect("Failed to open app.log for writing");

    let _ = WriteLogger::init(LevelFilter::Info, Config::default(), log_file);
}
