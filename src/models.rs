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
    pub fat: f32,
    pub standard_portion: f32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ConsumptionLog {
    pub id: Option<i64>,
    pub food_id: i64,
    pub quantity_in_grams: f32,
    pub consumtion_date: String,
}