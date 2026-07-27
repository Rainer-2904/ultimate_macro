mod models;
mod api;
mod db;
mod calc;

use chrono::Local;
use models::FoodItem;

#[tokio::main]
async fn main() {
    println!("--- Testing Delete and Update functions ---\n");

    // 1. Initialize local database
    if let Err(e) = db::init_db() {
        eprintln!("Failed to initialize database: {}", e);
        return;
    }

    // 2. Automatically get today's date in YYYY-MM-DD format
    let today = Local::now().format("%Y-%m-%d").to_string();
    println!("Today's date (auto): {}\n", today);

    // 3. Ensure we have a test product in local DB
    let test_barcode = "5941141000049"; // Zuzu Milk
    
    let food_id = match db::get_product_by_barcode(test_barcode) {
        Ok(Some(food)) => food.id.unwrap(),
        Ok(None) => {
            println!("Product not found locally, inserting test product...");
            let new_food = FoodItem {
                id: None,
                product_name: "Lapte Zuzu 1.5%".to_string(),
                brand: "Zuzu".to_string(),
                barcode: test_barcode.to_string(),
                kcal: 44.0,
                proteins: 3.2,
                carbohydrates: 4.7,
                fat: 1.5,
                standard_portion: 100.0,
            };
            db::insert_food(&new_food).expect("Failed to insert test food")
        }
        Err(e) => {
            eprintln!("Database error: {}", e);
            return;
        }
    };

    println!("Logging mistake: 5000g of milk");
    let log_id = db::log_food_consumption(food_id, 5000.0, &today).unwrap();
    println!("  => Created log entry with ID: {}", log_id);

    println!("\nCorrecting mistake: updating log ID {} to 250g...", log_id);
    match db::update_log_quantity(log_id, 250.0) {
        Ok(rows) if rows > 0 => println!("    Successfully updated {} row(s)!", rows),
        Ok(_) => println!("    Log entry with ID {} not found.", log_id),
        Err(e) => eprintln!("    Update failed: {}", e),
    }
    if let Ok(items) = db::get_logged_foods_for_date(&today) {
        println!("    Current log entries for today:");
        for item in items {
            if item.log_id == log_id {
                println!("      - ID [{}]: {} -> {:.1}g ({:.1} kcal)", item.log_id, item.product_name, item.quantity_in_grams, item.specific_macros.kcal);
            }
        }
    }
    println!("\nDeleting log ID {}...", log_id);
    match db::delete_log_entry(log_id) {
        Ok(rows) if rows > 0 => println!("    Successfully deleted {} row(s)!", rows),
        Ok(_) => println!("    Log entry with ID {} not found.", log_id),
        Err(e) => eprintln!("    Delete failed: {}", e),
    }
}