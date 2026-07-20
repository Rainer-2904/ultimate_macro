use serde::{Deserialize, Serialize};

// Item struct
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FoodItem {
    // 'Option' to autogenerate id when inserted into db
    pub id: Option<i64>,
    pub product_name: String,
    pub brand: String,
    // Barcode saved as string to avoid problems with codes that start with '0'
    pub barcode: String,
    pub kcal: f32,
    pub proteins: f32,
    pub carbohydrates: f32,
    pub fats: f32,
    pub per_standard_portion: f32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DailyLog {
    pub id: Option<i64>,
    pub food_id: i64,
    pub quantity: f32,
    pub consume_date: String,
}