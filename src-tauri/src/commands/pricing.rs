// Pricing catalog and model-rate commands.
use crate::*;

#[tauri::command]
pub(crate) fn get_token_history(days: u32) -> Result<token_history::TokenHistoryResponse, String> {
    token_history::get_token_history(days)
}

#[tauri::command]
pub(crate) fn get_pricing() -> Result<pricing::PricingResponse, String> {
    pricing::get_pricing()
}

#[tauri::command]
pub(crate) fn set_model_rate(
    request: pricing::SetModelRateRequest,
) -> Result<pricing::PricingResponse, String> {
    pricing::set_model_rate(request)
}

#[tauri::command]
pub(crate) fn reset_model_rate(model_id: String) -> Result<pricing::PricingResponse, String> {
    pricing::reset_model_rate(model_id)
}

#[tauri::command]
pub(crate) fn hide_model(model_id: String) -> Result<pricing::PricingResponse, String> {
    pricing::hide_model(model_id)
}

#[tauri::command]
pub(crate) fn unhide_model(model_id: String) -> Result<pricing::PricingResponse, String> {
    pricing::unhide_model(model_id)
}

#[tauri::command]
pub(crate) async fn refresh_pricing() -> Result<pricing::PricingResponse, String> {
    tauri::async_runtime::spawn_blocking(|| pricing::refresh_pricing_catalog(true))
        .await
        .map_err(|e| e.to_string())?
}
