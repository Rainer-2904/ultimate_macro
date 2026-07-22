use rusqlite::{params, Connection, Result, OptionalExtension};
use crate::models::{DailyMacroSummary, FoodItem};
use crate::calc;

// Open local db connection/Create it if it does not exist
pub fn get_connection() -> Result<Connection> {
    Connection::open("ultimate_macro.db")
}

// Initialize the db tables
pub fn init_db() -> Result<()> {
    let conn = get_connection()?;

    // Create foods table for product dictionary
    conn.execute(
        "CREATE TABLE IF NOT EXISTS foods (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        product_name TEXT NOT NULL,
        brand TEXT NOT NULL,
        barcode TEXT UNIQUE NOT NULL,
        kcal REAL NOT NULL,
        proteins REAL NOT NULL,
        carbohydrates REAL NOT NULL,
        fat REAL NOT NULL,
        standard_portion REAL NOT NULL
    )",
    [],
    )?;

    // Create log table for consumption history
    conn.execute(
        "CREATE TABLE IF NOT EXISTS consumption_log (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        food_id INTEGER NOT NULL,
        quantity_in_grams REAL NOT NULL,
        consumption_date TEXT NOT NULL,
        FOREIGN KEY (food_id) REFERENCES foods(id)
        )",
        [],
    )?;

    Ok(())
}

// Insert new 'FoodItem' in local db and return id
pub fn insert_food(food: &FoodItem) -> Result<i64> {
    let conn = get_connection()?;

    conn.execute(
        "INSERT INTO foods (product_name, brand, barcode, kcal, proteins, carbohydrates, fat, standard_portion)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7,?8)",
        params![
            food.product_name, food.brand, food.barcode,
            food.kcal, food.proteins, food.carbohydrates,
            food.fat, food.standard_portion
        ],
    )?;

    Ok(conn.last_insert_rowid())
}

// Search local db for item using barcode, returns 'Ok(None)' if barcode not found
// in local db to avoid returning an error, achieved trough ".optional()?"
pub fn get_product_by_barcode(barcode: &str) -> Result<Option<FoodItem>> {
    let conn = get_connection()?;
    let mut stmt = conn.prepare(
        "SELECT id, product_name, brand, barcode, kcal, proteins, carbohydrates, fat, standard_portion
        FROM foods WHERE barcode = ?1"
    )?;

    // Map SQL row to 'FoodItem' struct (models.rs)
    let food = stmt.query_row(params![barcode], |row| {
        Ok(FoodItem {
            id: row.get(0)?,
            product_name: row.get(1)?,
            brand: row.get(2)?,
            barcode: row.get(3)?,
            kcal: row.get(4)?,
            proteins: row.get(5)?,
            carbohydrates: row.get(6)?,
            fat: row.get(7)?,
            standard_portion: row.get(8)?,
        })
    }).optional()?;

    Ok(food)
}

pub fn get_daily_macros(date: &str) -> Result<DailyMacroSummary> {
    let conn = get_connection()?;

    let mut stmt = conn.prepare(
        "SELECT d.quantity_in_grams, f.kcal, f.proteins, f.carbohydrates, f.fat, f.standard_portion
        FROM consumption_log d
        JOIN food f ON d.food_id = f.id
        WHERE d.consumption_date = ?1"
    )?;

    // Map trough SQL rows and delegate the math to 'calc' module
    let rows = stmt.query_map(params![date], |row| {
        let quantity: f32 = row.get(0)?;
        let kcal: f32 = row.get(1)?;
        let proteins: f32 = row.get(2)?;
        let carbohydrates: f32 = row.get(3)?;
        let fat: f32 = row.get(4)?;
        let portion_size: f32 = row.get(5)?;

        Ok(calc::calculate_consumed_macros(
            quantity, portion_size, kcal, proteins, carbohydrates, fat
        ))
    })?;

    // Start from default values (0)
    let mut total = DailyMacroSummary::default();

    // Accumulate the results
    for row_result in rows {
        let entry_macro = row_result?;
        total = calc::add_summaries(total, entry_macro)
    }

    Ok(total)
}