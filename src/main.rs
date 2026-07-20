mod models;
mod api;

#[tokio::main]
async fn main() {
    println!("Testing API integration...\n");

    // 5060166697549 - white monster in Romania
    let test_barcode = "5060166697549";

    println!("Fetching barcode: {}...", test_barcode);

    match api::fetch_food_by_barcode(test_barcode).await {
        Ok(food) => {
            println!("Product found:");
            println!("   Name:  {} ({})", food.product_name, food.brand);
            println!("   Macros (per 100g):");
            println!("     - Kcal:   {:.1}", food.kcal);
            println!("     - Prot:   {:.1}g", food.proteins);
            println!("     - Carbs:  {:.1}g", food.carbohydrates);
            println!("     - Fats:   {:.1}g", food.fat);
        }
        Err(e) => {
            eprintln!("Error: {}", e)
        }
    }

    // 5941355009346 - Zuzu 1.5% Milk
    let test_barcode2 = "5941355009346";

    println!("Fetching barcode: {}...", test_barcode);

    match api::fetch_food_by_barcode(test_barcode2).await {
        Ok(food) => {
            println!("Product found:");
            println!("   Name:  {} ({})", food.product_name, food.brand);
            println!("   Macros (per 100g):");
            println!("     - Kcal:   {:.1}", food.kcal);
            println!("     - Prot:   {:.1}g", food.proteins);
            println!("     - Carbs:  {:.1}g", food.carbohydrates);
            println!("     - Fats:   {:.1}g", food.fat);
        }
        Err(e) => {
            eprintln!("Error: {}", e)
        }
    }
}
