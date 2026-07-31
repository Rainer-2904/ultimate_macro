use crate::models::DailyMacroSummary;

pub fn calculate_consumed_macros(
    consumed_quantity: f32,
    standard_portion: f32,
    base_kcal: f32,
    base_proteins: f32,
    base_carbohydrates: f32,
    base_fat: f32,
) -> DailyMacroSummary {
    let multiplier = consumed_quantity / standard_portion;

    DailyMacroSummary {
        kcal: base_kcal * multiplier,
        proteins: base_proteins * multiplier,
        carbohydrates: base_carbohydrates * multiplier,
        fat: base_fat* multiplier
    }
}

pub fn add_summaries(mut current_total: DailyMacroSummary, entry: DailyMacroSummary) -> DailyMacroSummary {
    current_total.kcal += entry.kcal;
    current_total.proteins += entry.proteins;
    current_total.carbohydrates += entry.carbohydrates;
    current_total.fat += entry.fat;

    current_total
}