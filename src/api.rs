use crate::models::FoodItem;
use reqwest::header::USER_AGENT;
use serde::Deserialize;

// --- API Response Structures ---
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
    let url = format!(
        "https://world.openfoodfacts.org/api/v0/product/{}.json",
        barcode
    );

    let builder = reqwest::Client::builder().timeout(std::time::Duration::from_secs(20));
    // Android's platform verifier rejects some valid server chains when OCSP
    // metadata is absent. Use Rustls/WebPKI with Mozilla roots consistently for
    // this public API; hostname, expiry, signatures and chain validation remain
    // enabled. Keep webpki-root-certs updated with application releases.
    #[cfg(target_os = "android")]
    let builder = builder.tls_certs_only(
        webpki_root_certs::TLS_SERVER_ROOT_CERTS
            .iter()
            .map(|certificate| reqwest::Certificate::from_der(certificate.as_ref()))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("Could not load certificate trust roots: {e}"))?,
    );
    let client = builder.build().map_err(|e| {
        log::error!("Failed to create HTTP client: {e:?}");
        "Could not initialize a secure connection.".to_string()
    })?;

    // Define User-Agent to comply with Open Food Facts API ToS
    // Format: AppName - System - Version - ContactInfo
    let custom_user_agent =
        "UltimateMacro - Android/Linux - Version 0.1 - https://github.com/Rainer-2904";

    // Make request with header
    let response = client
        .get(&url)
        .header(USER_AGENT, custom_user_agent)
        .send()
        .await
        .map_err(|e| {
            // Debug preserves the nested TLS/DNS cause; Display only shows the URL.
            log::error!("Open Food Facts request failed for {barcode}: {e:?}");
            if e.is_timeout() {
                "Open Food Facts timed out. Please retry.".to_string()
            } else {
                "Could not connect to Open Food Facts. Check your connection and retry.".to_string()
            }
        })?;

    // Do not attempt to parse an HTTP error page as a product response.
    if !response.status().is_success() {
        return Err(format!(
            "Open Food Facts returned HTTP {}. Please retry later.",
            response.status()
        ));
    }

    // Parse into 'OffResponse'
    let api_data = response.json::<OffResponse>().await.map_err(|e| {
        log::error!("Invalid Open Food Facts response for {barcode}: {e:?}");
        "Open Food Facts returned product data the app could not read.".to_string()
    })?;

    // Check if product is available in openfoodfacts db
    if api_data.status != 1 {
        return Err(format!("Product with barcode {} not found", barcode));
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
        favorite: false,
        id: None, // As it will be assigned automatically by SQLite
        product_name: product
            .product_name
            .unwrap_or_else(|| "Unknown Product".to_string()),
        brand: product
            .brands
            .unwrap_or_else(|| "Unknown Brand".to_string()),
        barcode: barcode.to_string(),
        kcal: nutriments.kcal.unwrap_or(0.0),
        proteins: nutriments.proteins.unwrap_or(0.0),
        carbohydrates: nutriments.carbohydrates.unwrap_or(0.0),
        fat: nutriments.fat.unwrap_or(0.0),
        standard_portion: 100.0, // OpenFoodFacts only uses per 100g, if using another DB check to make sure
    };
    Ok(food_item)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_known_zuzu_product_response() {
        // Relevant fields from OFF barcode 5941355009346. The reported device
        // failure was TLS setup, not an absent product or invalid nutrition data.
        let response: OffResponse = serde_json::from_str(
            r#"{
            "status": 1,
            "product": {
                "product_name": "lapte ZUZU  1,8l ..1,5grasime",
                "brands": "zuzu",
                "nutriments": {"energy-kcal_100g": 44, "proteins_100g": 3.1,
                    "carbohydrates_100g": 4.5, "fat_100g": 1.5}
            }
        }"#,
        )
        .unwrap();
        assert_eq!(response.status, 1);
        let product = response.product.unwrap();
        assert_eq!(product.brands.as_deref(), Some("zuzu"));
        let nutrients = product.nutriments.unwrap();
        assert_eq!(
            (
                nutrients.kcal,
                nutrients.proteins,
                nutrients.carbohydrates,
                nutrients.fat
            ),
            (Some(44.0), Some(3.1), Some(4.5), Some(1.5))
        );
    }
}
