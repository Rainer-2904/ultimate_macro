use crate::calc;
use crate::models::{DailyMacroGoal, DailyMacroSummary, FoodItem, LogDisplayItem};
use chrono::Local;
use rusqlite::{Connection, OptionalExtension, Result, params};
use std::time::{SystemTime, UNIX_EPOCH};

// Open local db connection/Create it if it does not exist
pub fn get_connection() -> Result<Connection> {
    Connection::open(crate::storage::path("ultimate_macro.db"))
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

    init_user_settings(&conn)?;
    Ok(())
}

// One settings row per local user. Preserve the legacy table, but copy its row
// only when settings are absent so restarting never overwrites newer goals.
fn init_user_settings(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS user_settings (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            target_kcal REAL NOT NULL CHECK (target_kcal >= 0),
            target_proteins REAL NOT NULL CHECK (target_proteins >= 0),
            target_carbohydrates REAL NOT NULL CHECK (target_carbohydrates >= 0),
            target_fat REAL NOT NULL CHECK (target_fat >= 0)
        );",
    )?;
    let legacy_exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'daily_goals')",
        [],
        |row| row.get(0),
    )?;
    if legacy_exists {
        conn.execute_batch(
            "INSERT OR IGNORE INTO user_settings
             (id, target_kcal, target_proteins, target_carbohydrates, target_fat)
             SELECT id, target_kcal, target_proteins, target_carbohydrates, target_fat
             FROM daily_goals WHERE id = 1;",
        )?;
    }
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
    let food = stmt
        .query_row(params![barcode], |row| {
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
        })
        .optional()?;
    Ok(food)
}

pub fn get_daily_macros(date: &str) -> Result<DailyMacroSummary> {
    let conn = get_connection()?;

    let mut stmt = conn.prepare(
        "SELECT d.quantity_in_grams, f.kcal, f.proteins, f.carbohydrates, f.fat, f.standard_portion
        FROM consumption_log d
        JOIN foods f ON d.food_id = f.id
        WHERE d.consumption_date = ?1",
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
            quantity,
            portion_size,
            kcal,
            proteins,
            carbohydrates,
            fat,
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

pub fn log_food_consumption(food_id: i64, quantity_in_grams: f32) -> Result<i64> {
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
         WHERE d.consumption_date = ?1",
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
            quantity,
            portion_size,
            kcal,
            proteins,
            carbohydrates,
            fat,
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

    let rows_affected =
        conn.execute("DELETE FROM consumption_log WHERE id = ?1", params![log_id])?;
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
    write_daily_goals(&get_connection()?, goal)
}

fn write_daily_goals(conn: &Connection, goal: &DailyMacroGoal) -> Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO user_settings (id, target_kcal, target_proteins, target_carbohydrates, target_fat)
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
    read_daily_goals(&get_connection()?)
}

fn read_daily_goals(conn: &Connection) -> Result<Option<DailyMacroGoal>> {
    let mut stmt = conn.prepare(
        "SELECT target_kcal, target_proteins, target_carbohydrates, target_fat 
            FROM user_settings WHERE id = 1",
    )?;

    let goal = stmt
        .query_row([], |row| {
            Ok(DailyMacroGoal {
                target_kcal: row.get(0)?,
                target_proteins: row.get(1)?,
                target_carbohydrates: row.get(2)?,
                target_fat: row.get(3)?,
            })
        })
        .optional()?; // Returns 'Ok(None)' if user hasn't set any goals
    Ok(goal)
}

#[cfg(test)]
mod settings_tests {
    use super::*;

    #[test]
    fn goals_persist_across_connections_and_replace_the_single_row() {
        // Use a temporary database rather than touching the user's working database.
        let path = std::env::temp_dir().join(format!(
            "ultimate-macro-settings-{}-{}.db",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        {
            let conn = Connection::open(&path).unwrap();
            init_user_settings(&conn).unwrap();
            assert!(read_daily_goals(&conn).unwrap().is_none());
            let mut goal = DailyMacroGoal {
                target_kcal: 2200.0,
                target_proteins: 120.5,
                target_carbohydrates: 260.0,
                target_fat: 0.0,
            };
            write_daily_goals(&conn, &goal).unwrap();
            goal.target_kcal = 2400.0;
            write_daily_goals(&conn, &goal).unwrap();
        }
        {
            let conn = Connection::open(&path).unwrap();
            init_user_settings(&conn).unwrap();
            let goal = read_daily_goals(&conn).unwrap().unwrap();
            assert_eq!(
                (
                    goal.target_kcal,
                    goal.target_proteins,
                    goal.target_carbohydrates,
                    goal.target_fat
                ),
                (2400.0, 120.5, 260.0, 0.0)
            );
            let count: i64 = conn
                .query_row("SELECT COUNT(*) FROM user_settings", [], |r| r.get(0))
                .unwrap();
            assert_eq!(count, 1);
        }
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn legacy_goals_migrate_without_overwriting_new_settings() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE daily_goals (id INTEGER PRIMARY KEY, target_kcal REAL,
            target_proteins REAL, target_carbohydrates REAL, target_fat REAL);
            INSERT INTO daily_goals VALUES (1, 2000, 100, 250, 60);",
        )
        .unwrap();
        init_user_settings(&conn).unwrap();
        let mut goal = read_daily_goals(&conn).unwrap().unwrap();
        assert_eq!(
            (
                goal.target_kcal,
                goal.target_proteins,
                goal.target_carbohydrates,
                goal.target_fat
            ),
            (2000.0, 100.0, 250.0, 60.0)
        );
        goal.target_kcal = 2300.0;
        write_daily_goals(&conn, &goal).unwrap();
        init_user_settings(&conn).unwrap();
        assert_eq!(
            read_daily_goals(&conn).unwrap().unwrap().target_kcal,
            2300.0
        );
    }
}
