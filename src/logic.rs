use crate::{api, db};

pub async fn handle_scanned_barcode(barcode: &str, quantity_in_grams: f32) {
    match db::get_product_by_barcode(barcode) {
        // Search local cache first for items (No internet required, just as God intended)
        Ok(Some(food)) => {
            db::log_food_consumption(food.id.unwrap(), quantity_in_grams).unwrap();
        }
        // Not found in local cache, we fetch it from API
        Ok(None) => {
            match api::fetch_food_by_barcode(barcode).await {
                Ok(new_food) => {
                    let new_id = db::insert_food(&new_food).unwrap(); // Save it to local db
                    db::log_food_consumption(new_id, quantity_in_grams).unwrap();
                }
                Err(e) => {
                    eprintln!("API Error or you are curently offline.");
                    eprintln!("[Debug] {}", e);
                }
            }
        }
        Err(e) => eprintln!("[DB error] {}", e),
    }
}