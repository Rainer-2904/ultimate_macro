#[cfg(target_os = "android")]
mod android;
mod api;
mod calc;
mod db;
mod logic;
mod models;
mod scanner;
mod storage;

use log::error;
use slint::{ModelRc, VecModel};

slint::include_modules!();

/// Reload today's dashboard from SQLite on the UI thread.
/// Call after successful consumption writes and, when implemented, log edits/deletes
/// or goal changes. There is currently no midnight timer to trigger a date rollover.
fn refresh_dashboard(ui: &MainWindow) {
    // Finish all reads before changing UI properties so a failed query leaves the
    // previous display intact. These separate queries are not a database snapshot.
    let result = (|| -> rusqlite::Result<_> {
        // Match the local ISO date used by db::log_food_consumption.
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        Ok((
            db::get_daily_macros(&today)?,
            db::get_logged_foods_for_date(&today)?,
            db::get_daily_goals()?,
        ))
    })();

    match result {
        Ok((totals, items, goals)) => {
            ui.set_totals(MacroValues {
                kcal: totals.kcal,
                proteins: totals.proteins,
                carbohydrates: totals.carbohydrates,
                fat: totals.fat,
            });
            // No saved goals means zero targets, not suggested nutritional goals.
            // Keep presence separate so the UI can explain why targets are absent.
            ui.set_has_goals(goals.is_some());
            ui.set_goals(
                goals
                    .map(|goal| MacroValues {
                        kcal: goal.target_kcal,
                        proteins: goal.target_proteins,
                        carbohydrates: goal.target_carbohydrates,
                        fat: goal.target_fat,
                    })
                    .unwrap_or_default(),
            );
            // Round only the display strings; totals above retain their precision.
            // Replace the whole Slint model on refresh to notify the list repeater.
            let rows: Vec<LogDisplayData> = items
                .into_iter()
                .map(|item| LogDisplayData {
                    name: item.product_name.into(),
                    details: format!("{}g · {}", item.quantity_in_grams, item.brand).into(),
                    calories: format!("{:.0}", item.specific_macros.kcal).into(),
                    proteins: format!("{:.0}", item.specific_macros.proteins).into(),
                    carbohydrates: format!("{:.0}", item.specific_macros.carbohydrates).into(),
                    fats: format!("{:.0}", item.specific_macros.fat).into(),
                })
                .collect();
            ui.set_food_log(ModelRc::new(VecModel::from(rows)));
            ui.set_dashboard_error("".into());
        }
        Err(e) => {
            error!("Failed to refresh dashboard: {}", e);
            ui.set_dashboard_error("Could not load today's food data.".into());
        }
    }
}

// Validate every field before writing: malformed input must never partially save.
fn parse_goal(value: &str, label: &str) -> Result<f32, String> {
    let number = value
        .trim()
        .parse::<f32>()
        .map_err(|_| format!("Enter a valid number for {label}."))?;
    if !number.is_finite() || number < 0.0 {
        return Err(format!("{label} must be a finite number of 0 or more."));
    }
    Ok(number)
}

#[tokio::main]
// Avoided using 'Result' on main to properly log errors and exit in a civilized manner.
// Otherwise, the error would be thrown to tokio instead
// and the program would exit with a non-zero code without any logging.
// WHICH IS NOT IDEAL.
pub async fn run() {
    logic::init_logger();

    if let Err(e) = db::init_db() {
        error!("Failed to initialize database: {}", e);
        return;
    }

    let ui = match MainWindow::new() {
        Ok(ui) => ui,
        Err(e) => {
            error!("Failed to create UI: {}", e);
            return;
        }
    };

    // Populate the window before its first render.
    refresh_dashboard(&ui);
    // Load editor drafts only at startup; dashboard refreshes must not erase edits.
    match db::get_daily_goals() {
        Ok(Some(goal)) => {
            ui.set_goal_proteins(goal.target_proteins.to_string().into());
            ui.set_goal_carbohydrates(goal.target_carbohydrates.to_string().into());
            ui.set_goal_fat(goal.target_fat.to_string().into());
            ui.set_goal_kcal(goal.target_kcal.to_string().into());
        }
        Ok(None) => {}
        Err(e) => {
            error!("Failed to load settings: {}", e);
            ui.set_settings_error(true);
            ui.set_settings_status("Could not load saved goals.".into());
        }
    }
    let settings_handle = ui.as_weak();
    ui.on_save_goals(move |proteins, carbohydrates, fat, kcal| {
        let Some(ui) = settings_handle.upgrade() else {
            return;
        };
        let result = (|| -> Result<(), String> {
            let goal = models::DailyMacroGoal {
                target_proteins: parse_goal(&proteins, "Proteins")?,
                target_carbohydrates: parse_goal(&carbohydrates, "Carbohydrates")?,
                target_fat: parse_goal(&fat, "Fats")?,
                target_kcal: parse_goal(&kcal, "Kcal")?,
            };
            db::set_daily_goals(&goal).map_err(|e| {
                error!("Failed to save goals: {}", e);
                "Could not save goals. Please try again.".to_string()
            })
        })();
        ui.set_settings_error(result.is_err());
        match result {
            Ok(()) => {
                refresh_dashboard(&ui);
                ui.set_settings_status("Daily goals saved.".into());
            }
            Err(message) => ui.set_settings_status(message.into()),
        }
    });
    let _scanner_timer = scanner::connect(&ui);

    if let Err(e) = ui.run() {
        error!("Failed to run UI: {}", e);
    }
}

#[cfg(test)]
mod settings_validation_tests {
    use super::parse_goal;

    #[test]
    fn accepts_zero_decimals_and_surrounding_whitespace() {
        assert_eq!(parse_goal("0", "Fats").unwrap(), 0.0);
        assert_eq!(parse_goal(" 120.5 ", "Proteins").unwrap(), 120.5);
    }

    #[test]
    fn rejects_missing_negative_and_nonfinite_goals() {
        for input in ["", "  ", "abc", "-1", "NaN", "inf", "-inf", "1e100"] {
            assert!(parse_goal(input, "Kcal").is_err(), "accepted {input}");
        }
    }
}
