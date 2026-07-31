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
pub struct DailyConsumptionLog {
    pub id: Option<i64>,
    pub food_id: i64,
    pub quantity_in_grams: f32,
    pub consumption_date: String,
}

#[derive(Debug, Default)]
pub struct DailyMacroSummary {
    pub kcal: f32,
    pub proteins: f32,
    pub carbohydrates: f32,
    pub fat: f32,
}

#[derive(Debug)]
pub struct LogDisplayItem {
    pub log_id: i64,
    pub product_name: String,
    pub brand: String,
    pub quantity_in_grams: f32,
    pub specific_macros: DailyMacroSummary,
}

#[derive(Debug, Clone)]
pub struct DailyMacroGoal {
    pub target_kcal: f32,
    pub target_proteins: f32,
    pub target_carbohydrates: f32,
    pub target_fat: f32,
}