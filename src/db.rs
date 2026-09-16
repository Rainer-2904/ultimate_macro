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
        standard_portion REAL NOT NULL,
        favorite INTEGER NOT NULL DEFAULT 0 CHECK (favorite IN (0, 1))
    )",
        [],
    )?;

    migrate_food_favorite(&conn)?;

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

// SQLite stores booleans as 0/1. Append the column to existing food dictionaries.
fn migrate_food_favorite(conn: &Connection) -> Result<()> {
    let exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM pragma_table_info('foods') WHERE name = 'favorite')",
        [],
        |row| row.get(0),
    )?;
    if !exists {
        conn.execute_batch(
            "ALTER TABLE foods ADD COLUMN favorite INTEGER NOT NULL DEFAULT 0 CHECK (favorite IN (0, 1));",
        )?;
    }
    Ok(())
}

pub fn toggle_food_favorite(food_id: i64) -> Result<usize> {
    toggle_favorite(&get_connection()?, food_id)
}

fn toggle_favorite(conn: &Connection, food_id: i64) -> Result<usize> {
    conn.execute("UPDATE foods SET favorite = NOT favorite WHERE id = ?1", params![food_id])
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
        "INSERT INTO foods (product_name, brand, barcode, kcal, proteins, carbohydrates, fat, standard_portion, favorite)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            food.product_name, food.brand, food.barcode,
            food.kcal, food.proteins, food.carbohydrates,
            food.fat, food.standard_portion, food.favorite
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

// Search local db for item using barcode, returns 'Ok(None)' if barcode not found
// in local db to avoid returning an error, achieved trough '.optional()?'
pub fn get_product_by_barcode(barcode: &str) -> Result<Option<FoodItem>> {
    let conn = get_connection()?;
    let mut stmt = conn.prepare(
        "SELECT id, product_name, brand, barcode, kcal, proteins, carbohydrates, fat, standard_portion, favorite
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
                favorite: row.get(9)?,
            })
        })
        .optional()?;
    Ok(food)
}

pub fn get_favorite_foods() -> Result<Vec<FoodItem>> {
    read_favorite_foods(&get_connection()?)
}

fn read_favorite_foods(conn: &Connection) -> Result<Vec<FoodItem>> {
    let mut stmt = conn.prepare(
        "SELECT id, product_name, brand, barcode, kcal, proteins, carbohydrates, fat, standard_portion, favorite
         FROM foods WHERE favorite = 1 ORDER BY product_name COLLATE NOCASE, id",
    )?;
    stmt.query_map([], |row| {
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
            favorite: row.get(9)?,
        })
    })?.collect()
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
            f.kcal, f.proteins, f.carbohydrates, f.fat, f.standard_portion, f.id, f.favorite
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
            food_id: row.get(9)?,
            favorite: row.get(10)?,
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

pub fn get_log_entry(log_id: i64) -> Result<Option<(String, f32)>> {
    get_connection()?.query_row(
        "SELECT f.product_name || ' · ' || f.brand, d.quantity_in_grams
         FROM consumption_log d JOIN foods f ON f.id = d.food_id WHERE d.id = ?1",
        [log_id], |row| Ok((row.get(0)?, row.get(1)?)),
    ).optional()
}

pub fn delete_log_entry(log_id: i64) -> Result<usize> {
    delete_entry(&get_connection()?, log_id)
}

fn delete_entry(conn: &Connection, log_id: i64) -> Result<usize> {

    let rows_affected =
        conn.execute("DELETE FROM consumption_log WHERE id = ?1", params![log_id])?;
    // Returns 1 if deleted, 0 if log_id was not found
    Ok(rows_affected)
}

pub fn update_log_quantity(log_id: i64, new_quantity_in_grams: f32) -> Result<usize> {
    update_quantity(&get_connection()?, log_id, new_quantity_in_grams)
}

fn update_quantity(conn: &Connection, log_id: i64, new_quantity_in_grams: f32) -> Result<usize> {

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
        favorite: false,
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

#[cfg(test)]
mod favorite_tests {
    use super::*;

    #[test]
    fn favorites_include_only_saved_foods_without_requiring_log_entries() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE foods (id INTEGER PRIMARY KEY, product_name TEXT, brand TEXT,
             barcode TEXT, kcal REAL, proteins REAL, carbohydrates REAL, fat REAL,
             standard_portion REAL, favorite INTEGER);
             INSERT INTO foods VALUES (1, 'Zucchini', 'Farm', '001', 20, 1, 3, 0, 100, 1),
             (2, 'Apple', 'Farm', '002', 52, 0, 14, 0, 100, 0),
             (3, 'banana', 'Farm', '003', 89, 1, 23, 0, 100, 1);"
        ).unwrap();
        let foods = read_favorite_foods(&conn).unwrap();
        assert_eq!(foods.iter().map(|food| food.id.unwrap()).collect::<Vec<_>>(), [3, 1]);
        assert_eq!(foods[0].standard_portion, 100.0);
        assert_eq!(foods[0].kcal, 89.0);
        assert!(foods.iter().all(|food| food.favorite));
        toggle_favorite(&conn, 3).unwrap();
        assert_eq!(read_favorite_foods(&conn).unwrap().len(), 1);
        toggle_favorite(&conn, 1).unwrap();
        assert!(read_favorite_foods(&conn).unwrap().is_empty());
    }


    #[test]
    fn legacy_foods_gain_a_persistent_boolean_without_losing_data() {
        let path = std::env::temp_dir().join(format!(
            "ultimate-macro-favorites-{}-{}.db", std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
        ));
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch(
                "CREATE TABLE foods (id INTEGER PRIMARY KEY, standard_portion REAL NOT NULL);
                 INSERT INTO foods VALUES (1, 100), (2, 50);"
            ).unwrap();
            migrate_food_favorite(&conn).unwrap();
            let original: (f32, bool) = conn.query_row(
                "SELECT standard_portion, favorite FROM foods WHERE id = 1", [],
                |row| Ok((row.get(0)?, row.get(1)?))
            ).unwrap();
            assert_eq!(original, (100.0, false));
            assert_eq!(toggle_favorite(&conn, 1).unwrap(), 1);
            assert_eq!(toggle_favorite(&conn, 999).unwrap(), 0);
            assert!(conn.execute("UPDATE foods SET favorite = 2 WHERE id = 1", []).is_err());
        }
        {
            let conn = Connection::open(&path).unwrap();
            migrate_food_favorite(&conn).unwrap();
            let read = |id| conn.query_row("SELECT favorite FROM foods WHERE id = ?1", [id], |row| row.get::<_, bool>(0)).unwrap();
            assert!(read(1));
            assert!(!read(2));
            toggle_favorite(&conn, 1).unwrap();
            assert!(!read(1));
            let columns: Vec<String> = conn.prepare("PRAGMA table_info(foods)").unwrap()
                .query_map([], |row| row.get(1)).unwrap().collect::<Result<_>>().unwrap();
            assert_eq!(columns, ["id", "standard_portion", "favorite"]);
        }
        std::fs::remove_file(path).unwrap();
    }
}

#[cfg(test)]
mod log_edit_tests {
    use super::*;

    #[test]
    fn editing_and_deleting_target_one_consumption_of_the_same_food() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE consumption_log (id INTEGER PRIMARY KEY, food_id INTEGER, quantity_in_grams REAL);
             INSERT INTO consumption_log VALUES (1, 7, 100), (2, 7, 200);"
        ).unwrap();
        assert_eq!(update_quantity(&conn, 1, 125.5).unwrap(), 1);
        let quantities: Vec<f32> = conn.prepare("SELECT quantity_in_grams FROM consumption_log ORDER BY id").unwrap()
            .query_map([], |row| row.get(0)).unwrap().collect::<Result<_>>().unwrap();
        assert_eq!(quantities, [125.5, 200.0]);
        assert_eq!(delete_entry(&conn, 1).unwrap(), 1);
        assert_eq!(delete_entry(&conn, 1).unwrap(), 0);
        assert_eq!(update_quantity(&conn, 1, 50.0).unwrap(), 0);
        let remaining: (i64, f32) = conn.query_row("SELECT id, quantity_in_grams FROM consumption_log", [],
            |row| Ok((row.get(0)?, row.get(1)?))).unwrap();
        assert_eq!(remaining, (2, 200.0));
    }
}
