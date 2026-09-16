use crate::{MainWindow, logic, models::FoodItem};
use slint::{ComponentHandle, Timer};
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct State {
    generation: u64,
    food: Option<FoodItem>,
    log_id: Option<i64>,
}

// A generation invalidates in-flight lookups when the form is closed or edited.
// Late network results may populate the cache, but must never restore stale UI.
fn reset(state: &Mutex<State>) -> u64 {
    let mut state = state.lock().unwrap();
    state.generation += 1;
    state.food = None;
    state.log_id = None;
    state.generation
}

fn validate_barcode(input: &str) -> Result<&str, &'static str> {
    let barcode = input.trim();
    if !matches!(barcode.len(), 8 | 12 | 13 | 14) || !barcode.bytes().all(|c| c.is_ascii_digit()) {
        return Err("Enter an 8, 12, 13, or 14 digit product barcode.");
    }
    // Keep it as a string: leading zeroes are significant product identifiers.
    Ok(barcode)
}

fn quantity(input: &str) -> Result<f32, &'static str> {
    let value = input
        .trim()
        .parse::<f32>()
        .map_err(|_| "Enter a quantity in grams.")?;
    if !value.is_finite() || value <= 0.0 {
        return Err("Quantity must be a finite number greater than zero.");
    }
    Ok(value)
}

fn lookup(ui: &MainWindow, state: Arc<Mutex<State>>, input: &str) {
    if ui.get_scanner_busy() {
        return;
    }
    let generation = reset(&state);
    ui.set_scanner_ready(false);
    let barcode = match validate_barcode(input) {
        Ok(barcode) => barcode.to_owned(),
        Err(message) => {
            ui.set_scanner_status_text(message.into());
            return;
        }
    };
    ui.set_scanner_barcode(barcode.clone().into());
    ui.set_scanner_busy(true);
    ui.set_scanner_status_text("Looking up product…".into());
    let handle = ui.as_weak();
    tokio::spawn(async move {
        let result = logic::lookup_food(&barcode).await;
        let _ = handle.upgrade_in_event_loop(move |ui| {
            let mut state = state.lock().unwrap();
            if state.generation != generation || !ui.get_scanner_visible() {
                return;
            }
            ui.set_scanner_busy(false);
            match result {
                Ok(food) => {
                    ui.set_scanner_product(
                        format!("{} · {}", food.product_name, food.brand).into(),
                    );
                    ui.set_scanner_status_text(
                        "Product found. Confirm how much you consumed.".into(),
                    );
                    state.food = Some(food);
                    ui.set_scanner_ready(true);
                }
                Err(error) => {
                    log::error!("Barcode lookup failed: {error}");
                    ui.set_scanner_status_text(error.into());
                }
            }
        });
    });
}

pub fn connect(ui: &MainWindow) -> Timer {
    let state = Arc::new(Mutex::new(State::default()));
    ui.set_camera_available(cfg!(target_os = "android"));
    let weak = ui.as_weak();
    let shared = state.clone();
    ui.on_open_scanner(move || {
        if let Some(ui) = weak.upgrade() {
            reset(&shared);
            ui.set_scanner_visible(true);
            ui.set_scanner_editing(false);
            ui.set_scanner_busy(false);
            ui.set_scanner_ready(false);
            ui.set_scanner_barcode("".into());
            ui.set_scanner_quantity("".into());
            ui.set_scanner_product("".into());
            ui.set_scanner_status_text("Scan or enter a product barcode.".into());
        }
    });
    let weak = ui.as_weak();
    let shared = state.clone();
    ui.on_open_favorite(move |barcode| {
        let Some(ui) = weak.upgrade() else { return; };
        ui.invoke_open_scanner();
        ui.set_scanner_barcode(barcode.clone());
        // Favorites are already cached, including custom foods with nonnumeric barcodes.
        match crate::db::get_product_by_barcode(&barcode) {
            Ok(Some(food)) => {
                ui.set_scanner_product(format!("{} · {}", food.product_name, food.brand).into());
                ui.set_scanner_status_text("Product found. Confirm how much you consumed.".into());
                shared.lock().unwrap().food = Some(food);
                ui.set_scanner_ready(true);
            }
            Ok(None) => ui.set_scanner_status_text("This food is no longer available.".into()),
            Err(error) => {
                log::error!("Failed to load favorite: {error}");
                ui.set_scanner_status_text("Could not load this food. Please try again.".into());
            }
        }
    });
    let weak = ui.as_weak();
    let shared = state.clone();
    ui.on_edit_log(move |log_id| {
        let Some(ui) = weak.upgrade() else { return; };
        let Ok(log_id) = log_id.parse::<i64>() else { return; };
        ui.invoke_open_scanner();
        ui.set_scanner_editing(true);
        match crate::db::get_log_entry(log_id) {
            Ok(Some((product, grams))) => {
                shared.lock().unwrap().log_id = Some(log_id);
                ui.set_scanner_product(product.into());
                ui.set_scanner_quantity(grams.to_string().into());
                ui.set_scanner_status_text("Update the consumed quantity or delete this entry.".into());
                ui.set_scanner_ready(true);
            }
            Ok(None) => ui.set_scanner_status_text("This log entry no longer exists.".into()),
            Err(error) => {
                log::error!("Failed to load log entry: {error}");
                ui.set_scanner_status_text("Could not load this entry. Please try again.".into());
            }
        }
    });
    let weak = ui.as_weak();
    let shared = state.clone();
    ui.on_delete_log(move || {
        let Some(ui) = weak.upgrade() else { return; };
        if !ui.get_scanner_visible() || ui.get_scanner_busy() { return; }
        let log_id = shared.lock().unwrap().log_id;
        let Some(log_id) = log_id else { return; };
        match crate::db::delete_log_entry(log_id) {
            Ok(1) => {
                ui.invoke_close_scanner();
                crate::refresh_dashboard(&ui);
            }
            Ok(_) => ui.set_scanner_status_text("This log entry no longer exists.".into()),
            Err(error) => {
                log::error!("Failed to delete log entry: {error}");
                ui.set_scanner_status_text("Could not delete this entry. Please try again.".into());
            }
        }
    });
    let weak = ui.as_weak();
    let shared = state.clone();
    ui.on_close_scanner(move || {
        reset(&shared);
        if let Some(ui) = weak.upgrade() {
            ui.set_scanner_visible(false);
            ui.set_scanner_busy(false);
            ui.set_scanner_ready(false);
        }
    });
    let weak = ui.as_weak();
    let shared = state.clone();
    ui.on_barcode_edited(move || {
        reset(&shared);
        if let Some(ui) = weak.upgrade() {
            ui.set_scanner_ready(false);
        }
    });
    let weak = ui.as_weak();
    let shared = state.clone();
    ui.on_lookup_barcode(move |barcode| {
        if let Some(ui) = weak.upgrade() {
            lookup(&ui, shared.clone(), &barcode);
        }
    });
    let weak = ui.as_weak();
    let shared = state.clone();
    ui.on_confirm_food(move |input| {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        if ui.get_scanner_busy() || !ui.get_scanner_visible() {
            return;
        }
        let grams = match quantity(&input) {
            Ok(value) => value,
            Err(message) => {
                ui.set_scanner_status_text(message.into());
                return;
            }
        };
        let log_id = shared.lock().unwrap().log_id;
        if let Some(log_id) = log_id {
            match crate::db::update_log_quantity(log_id, grams) {
                Ok(1) => {
                    ui.invoke_close_scanner();
                    crate::refresh_dashboard(&ui);
                }
                Ok(_) => ui.set_scanner_status_text("This log entry no longer exists.".into()),
                Err(error) => {
                    log::error!("Failed to update log quantity: {error}");
                    ui.set_scanner_status_text("Could not save quantity. Please try again.".into());
                }
            }
            return;
        }
        let mut state = shared.lock().unwrap();
        let Some(food_id) = state.food.as_ref().and_then(|food| food.id) else {
            return;
        };
        // One confirmed write, then clear the selection before another click can log it.
        match crate::db::log_food_consumption(food_id, grams) {
            Ok(_) => {
                state.food = None;
                state.generation += 1;
                ui.set_scanner_ready(false);
                ui.set_scanner_visible(false);
                crate::refresh_dashboard(&ui);
            }
            Err(error) => {
                log::error!("Failed to log food: {error}");
                ui.set_scanner_status_text("Could not save food. Please retry.".into());
            }
        }
    });

    let timer = Timer::default();
    #[cfg(target_os = "android")]
    {
        let weak = ui.as_weak();
        let shared = state.clone();
        ui.on_start_camera(move || {
            let Some(ui) = weak.upgrade() else {
                return;
            };
            if ui.get_scanner_busy() {
                return;
            }
            let generation = reset(&shared);
            ui.set_scanner_ready(false);
            match crate::android::start_scan(generation) {
                Ok(()) => {
                    ui.set_scanner_busy(true);
                    ui.set_scanner_status_text("Camera open…".into());
                }
                Err(error) => ui.set_scanner_status_text(error.into()),
            }
        });
        let weak = ui.as_weak();
        timer.start(
            slint::TimerMode::Repeated,
            std::time::Duration::from_millis(150),
            move || {
                let Some(ui) = weak.upgrade() else {
                    return;
                };
                // Drain cancelled results as well so reopening never uses an old barcode.
                match crate::android::take_scan_result() {
                    Ok(Some(result)) if ui.get_scanner_visible() => {
                        // A result from a camera opened by a previous form is stale.
                        let Some((token, result)) = result.split_once('|') else {
                            return;
                        };
                        if token.parse::<u64>().ok() != Some(state.lock().unwrap().generation) {
                            return;
                        }
                        ui.set_scanner_busy(false);
                        if let Some(barcode) = result.strip_prefix("OK:") {
                            lookup(&ui, state.clone(), barcode);
                        } else if let Some(message) = result.strip_prefix("ERROR:") {
                            ui.set_scanner_status_text(message.into());
                        } else {
                            ui.set_scanner_status_text(
                                "Scan cancelled. Scan again or enter a barcode.".into(),
                            );
                        }
                    }
                    Err(error) if ui.get_scanner_busy() => {
                        ui.set_scanner_busy(false);
                        ui.set_scanner_status_text(error.into());
                    }
                    _ => {}
                }
            },
        );
    }
    timer
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn barcode_preserves_leading_zeroes_and_rejects_non_products() {
        assert_eq!(
            validate_barcode(" 0123456789012 ").unwrap(),
            "0123456789012"
        );
        for value in ["", "123", "https://example.org", "1234567a"] {
            assert!(validate_barcode(value).is_err());
        }
    }
    #[test]
    fn confirmation_requires_positive_finite_grams() {
        assert_eq!(quantity(" 12.5 ").unwrap(), 12.5);
        for value in ["", "0", "-1", "NaN", "inf", "1e100"] {
            assert!(quantity(value).is_err());
        }
    }
    #[test]
    fn closing_or_editing_invalidates_pending_lookup() {
        let state = Mutex::new(State::default());
        state.lock().unwrap().log_id = Some(42);
        let first = reset(&state);
        assert!(state.lock().unwrap().log_id.is_none());
        assert_ne!(first, reset(&state));
        assert!(state.lock().unwrap().food.is_none());
    }
}
