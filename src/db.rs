use rusqlite::{params, Connection, Result, OptionalExtension};
use std::time::{SystemTime, UNIX_EPOCH};
use chrono::Local;
use crate::models::{DailyMacroSummary, FoodItem, LogDisplayItem, DailyMacroGoal};
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

    // Create the daily_goals table for user settings
    conn.execute(
        "CREATE TABLE IF NOT EXISTS daily_goals (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            target_kcal REAL NOT NULL,
            target_proteins REAL NOT NULL,
            target_carbohydrates REAL NOT NULL,
            target_fat REAL NOT NULL
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
// in local db to avoid returning an error, achieved trough '.optional()?'
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
        JOIN foods f ON d.food_id = f.id
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

pub fn log_food_consumption(food_id: i64, quantity_in_grams: f32,) -> Result<i64> {
    let conn = get_connection()?;
    // Don't change the format, keep it ISO 8601 to avoid sorting problems down the line.
    // You can parse it to be displayed in another format for the UI if you want for example 'day-month-year'
    let today = Local::now().format("%Y-%m-%d").to_string();

    conn.execute(
        "INSERT INTO consumption_log (food_id, quantity_in_grams, consumption_date)
        VALUES (?1, ?2, ?3)",
        params![food_id, quantity_in_grams, today],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn get_logged_foods_for_date(date: &str) -> Result<Vec<LogDisplayItem>> {
    let conn = get_connection()?;

    let mut stmt = conn.prepare(
        "SELECT d.id, f.product_name, f.brand, d.quantity_in_grams, 
            f.kcal, f.proteins, f.carbohydrates, f.fat, f.standard_portion
         FROM consumption_log d
         JOIN foods f ON d.food_id = f.id
         WHERE d.consumption_date = ?1"
    )?;

    let rows = stmt.query_map(params![date], |row| {
        let log_id: i64 = row.get(0)?;
        let product_name: String = row.get(1)?;
        let brand: String = row.get(2)?;
        let quantity: f32 = row.get(3)?;
        
        let kcal: f32 = row.get(4)?;
        let proteins: f32 = row.get(5)?;
        let carbohydrates: f32 = row.get(6)?;
        let fat: f32 = row.get(7)?;
        let portion_size: f32 = row.get(8)?;

        let specific_macros = calc::calculate_consumed_macros(
            quantity, portion_size, kcal, proteins, carbohydrates, fat
        );

        Ok(LogDisplayItem {
            log_id,
            product_name,
            brand,
            quantity_in_grams: quantity,
            specific_macros,
        })
    })?;

    let mut daily_items = Vec::new();
    for item in rows {
        daily_items.push(item?);
    }
    Ok(daily_items)
}

pub fn delete_log_entry(log_id: i64) -> Result<usize> {
    let conn = get_connection()?;

    let rows_affected = conn.execute(
        "DELETE FROM consumption_log WHERE id = ?1",
        params![log_id],
    )?;
    // Returns 1 if deleted, 0 if log_id was not found
    Ok(rows_affected)
}

pub fn update_log_quantity(log_id: i64, new_quantity_in_grams: f32) -> Result<usize> {
    let conn = get_connection()?;

    let rows_affected = conn.execute(
        "UPDATE consumption_log SET quantity_in_grams = ?1 WHERE id =?2",
        params![new_quantity_in_grams, log_id],
    )?;
    // Returns number of affected rows
    Ok(rows_affected)
}

pub fn insert_custom_food(
    product_name: &str,
    kcal: f32,
    proteins: f32,
    carbohydrates: f32,
    fat: f32,
    standard_portion: f32,
) -> Result<i64> {
    /* 
    We generate a unique string based on timestamp since the barcode
    column in the local db has 'UNIQUE NOT NULL' property,
    meaning that the barcode field cannot be empty.
    Cheap hack but it will do since I have 0 intention to rethink the SQL
    and changing the property will surely fuck something up.
    */
    let timestamp = SystemTime::now() // Also store it as u64 because 2038 is just around the corner
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_secs();

    let custom_barcode = format!("Custom_{}", timestamp);

    let custom_food = FoodItem {
        id: None,
        product_name: product_name.to_string(),
        brand: "Homemade / Raw".to_string(),
        barcode: custom_barcode,
        kcal,
        proteins,
        carbohydrates,
        fat,
        standard_portion,
    };
    insert_food(&custom_food)
}

pub fn set_daily_goals(goal: &DailyMacroGoal) -> Result<()> {
    let conn = get_connection()?;

    conn.execute(
        "INSERT OR REPLACE INTO daily_goals (id, target_kcal, target_proteins, target_carbohydrates, target_fat)
            VALUES (1, ?1, ?2, ?3, ?4)",
        params![
            goal.target_kcal,
            goal.target_proteins,
            goal.target_carbohydrates,
            goal.target_fat
        ],
    )?;
    Ok(())
}

pub fn get_daily_goals() -> Result<Option<DailyMacroGoal>> {
    let conn = get_connection()?;

    let mut stmt = conn.prepare(
        "SELECT target_kcal, target_proteins, target_carbohydrates, target_fat 
            FROM daily_goals WHERE id = 1"
    )?;

    let goal = stmt.query_row([], |row| {
        Ok(DailyMacroGoal {
            target_kcal: row.get(0)?,
            target_proteins: row.get(1)?,
            target_carbohydrates: row.get(2)?,
            target_fat: row.get(3)?,
        })
    }).optional()?; // Returns 'Ok(None)' if user hasn't set any goals
    Ok(goal)
}