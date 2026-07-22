mod models;
mod api;
mod db;
mod calc;

use models::FoodItem;

#[tokio::main]
async fn main() {
    println!("--- DB TEST ---\n");

    match db::init_db() {
        Ok(_) => println!("Init successful"),
        Err(e) => {
            eprintln!("Error: {}", e);
            return;
        }
    }

    let test_barcode = "1234567890";
    let test_food = FoodItem {
        id: None,
        product_name: "Test Product".to_string(),
        brand: "Test Brand".to_string(),
        barcode: test_barcode.to_string(),
        kcal: 300.0,
        proteins: 20.4,
        carbohydrates: 25.0,
        fat: 1.0,
        standard_portion:100.0,
    };

    println!("Searching local db for barcode {}", test_barcode);

    match db::get_product_by_barcode(test_barcode) {
        Ok(Some(food)) => {
            println!("Found product in internal DB: {}", food.id.unwrap());
        }
        Ok(None) => {
            println!("Product not found in internal DB, inserting now...");

            match db::insert_food(&test_food) {
                Ok(new_id) => println!("Insert successful, item saved with id: {}", new_id),
                Err(e) => eprintln!("Error: {}", e),
            }
        }
        Err(e) => eprintln!("DB read error: {}", e),
    }

    if let Ok(Some(saved_food)) = db::get_product_by_barcode(test_barcode) {
        println!("Data extracted out of local db: {} ({})", saved_food.product_name, saved_food.brand);
        println!("     - Kcal:   {:.1}", saved_food.kcal);
        println!("     - Prot:   {:.1}g", saved_food.proteins);
    }
}
