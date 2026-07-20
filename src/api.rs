use crate::models::FoodItem;
use serde::Deserialize;

// --- API Responce Structures ---
// Using serde we can ignore the massive amount of data we get 
// from openfoodfacts.org/api and only extract what we need
//
// If you want to get more information you can check a random barcode against
// the API to see the exact field names

#[derive(Deserialize, Debug)]
struct OffResponse {
    status: i32,
    product: Option<OffProduct>,
}

#[derive(Deserialize, Debug)]
struct OffProduct {
    product_name: Option<String>,
    brands: Option<String>,
    nutriments: Option<OffNutriments>,
}

#[derive(Deserialize, Debug)]
struct OffNutriments {
    #[serde(rename = "energy-kcal_100g")]
    kcal: Option<f32>,
    #[serde(rename = "proteins_100g")]
    proteins: Option<f32>,
    #[serde(rename = "carbohydrates_100g")]
    carbohydrates: Option<f32>,
    #[serde(rename = "fat_100g")]
    fat: Option<f32>,
}

// Fetch data from API using a barcode
pub async fn fetch_food_by_barcode(barcode: &str) -> Result<FoodItem, String> {
    let url = format!("https://world.openfoodfacts.org/api/v0/product/{}.json", barcode);

    // HTTP get request
    let response = reqwest::get(&url) 
    .await
    .map_err(|e| format!("Failed to send request: {}", e))?;

    // Parse into 'OffResponse'
    let api_data = response
    .json::<OffResponse>()
    .await
    .map_err(|e| format!("Failed to parse JSON: {}", e))?;

    // Check if product is available in openfoodfacts db
    if api_data.status != 1 {
        return Err(format!("Product with barcode {} not found", barcode))
    }

    let product = api_data
    .product
    .ok_or_else(|| "Product data is missing from response".to_string())?;

    let nutriments = product.nutriments.unwrap_or(OffNutriments {
        kcal: None,
        proteins: None,
        carbohydrates: None,
        fat: None,
    });


    // Map the API data to the structure in models.rs
    // Using unwrap_or as a fallback in case of incomplete labels
    let food_item = FoodItem {
        id: None, // As it will be assigned automatically by SQLite
        product_name: product.product_name.unwrap_or_else(|| "Unknown Product".to_string()),
        brand: product.brands.unwrap_or_else(|| "Unknown Brand".to_string()),
        barcode: barcode.to_string(),
        kcal: nutriments.kcal.unwrap_or(0.0),
        proteins: nutriments.proteins.unwrap_or(0.0),
        carbohydrates: nutriments.carbohydrates.unwrap_or(0.0),
        fat: nutriments.fat.unwrap_or(0.0),
        standard_portion: 100.0, // OpenFoodFacts only uses per 100g, if using another DB check to make sure
    };
    Ok(food_item)
}