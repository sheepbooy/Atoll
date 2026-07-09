use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use crate::TokenUsage;

pub(crate) const UNKNOWN_MODEL: &str = "_unknown";

pub static LAST_REFRESH_ERROR: Mutex<Option<String>> = Mutex::new(None);

pub const CATALOG_STALE_SECS: u64 = 24 * 60 * 60;
pub const CATALOG_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ModelRate {
    pub input_per_million: f64,
    pub output_per_million: f64,
    pub cache_read_per_million: f64,
    pub cache_write_per_million: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelPricingEntry {
    pub model_id: String,
    pub display_name: String,
    pub rate: ModelRate,
    pub is_custom: bool,
    pub is_unpriced: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PricingResponse {
    pub models: Vec<ModelPricingEntry>,
    pub catalog_fetched_at: Option<String>,
    pub last_refresh_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct PricingOverridesFile {
    overrides: HashMap<String, ModelRateOverride>,
    hidden_model_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ModelRateOverride {
    pub display_name: Option<String>,
    #[serde(flatten)]
    pub rate: ModelRate,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CatalogModel {
    pub model_id: String,
    pub display_name: String,
    pub rate: ModelRate,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct PricingCatalogFile {
    pub version: u32,
    pub updated_at: Option<String>,
    pub fetched_at: Option<String>,
    pub etag: Option<String>,
    pub models: Vec<CatalogModel>,
}

#[derive(Debug, Clone)]
struct BuiltinModel {
    model_id: &'static str,
    display_name: &'static str,
    rate: ModelRate,
}

fn builtin_models() -> Vec<BuiltinModel> {
    vec![
        BuiltinModel {
            model_id: "claude-sonnet-4-20250514",
            display_name: "Claude Sonnet 4",
            rate: ModelRate {
                input_per_million: 3.0,
                output_per_million: 15.0,
                cache_read_per_million: 0.30,
                cache_write_per_million: 3.75,
            },
        },
        BuiltinModel {
            model_id: "claude-opus-4-20250514",
            display_name: "Claude Opus 4",
            rate: ModelRate {
                input_per_million: 15.0,
                output_per_million: 75.0,
                cache_read_per_million: 1.50,
                cache_write_per_million: 18.75,
            },
        },
        BuiltinModel {
            model_id: "claude-3-5-haiku-20241022",
            display_name: "Claude Haiku 3.5",
            rate: ModelRate {
                input_per_million: 0.80,
                output_per_million: 4.0,
                cache_read_per_million: 0.08,
                cache_write_per_million: 1.0,
            },
        },
        BuiltinModel {
            model_id: "gpt-4o",
            display_name: "GPT-4o",
            rate: ModelRate {
                input_per_million: 2.50,
                output_per_million: 10.0,
                cache_read_per_million: 1.25,
                cache_write_per_million: 2.50,
            },
        },
        BuiltinModel {
            model_id: "gpt-4o-mini",
            display_name: "GPT-4o mini",
            rate: ModelRate {
                input_per_million: 0.15,
                output_per_million: 0.60,
                cache_read_per_million: 0.075,
                cache_write_per_million: 0.15,
            },
        },
        BuiltinModel {
            model_id: "o3",
            display_name: "OpenAI o3",
            rate: ModelRate {
                input_per_million: 10.0,
                output_per_million: 40.0,
                cache_read_per_million: 2.50,
                cache_write_per_million: 10.0,
            },
        },
        BuiltinModel {
            model_id: "o4-mini",
            display_name: "OpenAI o4-mini",
            rate: ModelRate {
                input_per_million: 1.10,
                output_per_million: 4.40,
                cache_read_per_million: 0.275,
                cache_write_per_million: 1.10,
            },
        },
        BuiltinModel {
            model_id: "gpt-4.1",
            display_name: "GPT-4.1",
            rate: ModelRate {
                input_per_million: 2.0,
                output_per_million: 8.0,
                cache_read_per_million: 0.50,
                cache_write_per_million: 2.0,
            },
        },
        BuiltinModel {
            model_id: "gpt-4.1-mini",
            display_name: "GPT-4.1 mini",
            rate: ModelRate {
                input_per_million: 0.40,
                output_per_million: 1.60,
                cache_read_per_million: 0.10,
                cache_write_per_million: 0.40,
            },
        },
    ]
}

pub fn pricing_path() -> Option<PathBuf> {
    if let Ok(path) = std::env::var("ATOLL_PRICING_PATH") {
        if !path.is_empty() {
            return Some(PathBuf::from(path));
        }
    }
    dirs::home_dir().map(|home| home.join(".atoll").join("pricing.json"))
}

pub fn catalog_path() -> Option<PathBuf> {
    if let Ok(path) = std::env::var("ATOLL_PRICING_CATALOG_PATH") {
        if !path.is_empty() {
            return Some(PathBuf::from(path));
        }
    }
    dirs::home_dir().map(|home| home.join(".atoll").join("pricing-catalog.json"))
}

pub fn validate_catalog(file: &PricingCatalogFile) -> Result<(), String> {
    if file.version != CATALOG_SCHEMA_VERSION {
        return Err(format!("unsupported catalog version {}", file.version));
    }
    for model in &file.models {
        if model.model_id.trim().is_empty() {
            return Err("catalog modelId must be non-empty".into());
        }
        for value in [
            model.rate.input_per_million,
            model.rate.output_per_million,
            model.rate.cache_read_per_million,
            model.rate.cache_write_per_million,
        ] {
            if !value.is_finite() || value < 0.0 {
                return Err(format!("invalid rate for {}", model.model_id));
            }
        }
    }
    Ok(())
}

pub fn load_catalog() -> PricingCatalogFile {
    let Some(path) = catalog_path() else {
        return PricingCatalogFile::default();
    };
    let content = match std::fs::read_to_string(&path) {
        Ok(content) => content,
        Err(_) => return PricingCatalogFile::default(),
    };
    match serde_json::from_str::<PricingCatalogFile>(&content) {
        Ok(file) if validate_catalog(&file).is_ok() => file,
        _ => PricingCatalogFile::default(),
    }
}

pub fn save_catalog(file: &PricingCatalogFile) -> Result<(), String> {
    validate_catalog(file)?;
    let Some(path) = catalog_path() else {
        return Ok(());
    };
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let formatted = serde_json::to_string_pretty(file).map_err(|error| error.to_string())?;
    std::fs::write(path, formatted).map_err(|error| error.to_string())
}

fn load_overrides() -> PricingOverridesFile {
    let Some(path) = pricing_path() else {
        return PricingOverridesFile::default();
    };
    let content = match std::fs::read_to_string(&path) {
        Ok(content) => content,
        Err(_) => return PricingOverridesFile::default(),
    };
    serde_json::from_str(&content).unwrap_or_default()
}

fn save_overrides(file: &PricingOverridesFile) -> Result<(), String> {
    let Some(path) = pricing_path() else {
        return Ok(());
    };
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let formatted = serde_json::to_string_pretty(file).map_err(|error| error.to_string())?;
    std::fs::write(path, formatted).map_err(|error| error.to_string())
}

fn builtin_rate(model_id: &str) -> Option<ModelRate> {
    builtin_models()
        .into_iter()
        .find(|model| model.model_id == model_id)
        .map(|model| model.rate)
}

fn builtin_display_name(model_id: &str) -> Option<&'static str> {
    builtin_models()
        .into_iter()
        .find(|model| model.model_id == model_id)
        .map(|model| model.display_name)
}

pub fn effective_rate(model_id: &str) -> Option<ModelRate> {
    let overrides = load_overrides();
    if let Some(override_rate) = overrides.overrides.get(model_id) {
        return Some(override_rate.rate);
    }
    let catalog = load_catalog();
    if let Some(model) = catalog.models.iter().find(|m| m.model_id == model_id) {
        return Some(model.rate);
    }
    builtin_rate(model_id)
}

pub fn usage_cost_usd(usage: TokenUsage, rate: ModelRate) -> f64 {
    (usage.input_tokens as f64 * rate.input_per_million
        + usage.output_tokens as f64 * rate.output_per_million
        + usage.cache_read_tokens as f64 * rate.cache_read_per_million
        + usage.cache_creation_tokens as f64 * rate.cache_write_per_million)
        / 1_000_000.0
}

pub fn by_model_cost_usd(by_model: &HashMap<String, TokenUsage>) -> f64 {
    by_model
        .iter()
        .filter(|(model_id, _)| model_id.as_str() != UNKNOWN_MODEL)
        .filter_map(|(model_id, usage)| {
            effective_rate(model_id).map(|rate| usage_cost_usd(*usage, rate))
        })
        .sum()
}

fn catalog_display_name(model_id: &str, catalog: &PricingCatalogFile) -> Option<String> {
    catalog
        .models
        .iter()
        .find(|model| model.model_id == model_id)
        .map(|model| model.display_name.clone())
}

fn zero_rate() -> ModelRate {
    ModelRate {
        input_per_million: 0.0,
        output_per_million: 0.0,
        cache_read_per_million: 0.0,
        cache_write_per_million: 0.0,
    }
}

pub fn get_pricing() -> Result<PricingResponse, String> {
    let discovered = crate::token_history::known_model_ids();
    get_pricing_with_discovered(discovered)
}

pub fn get_pricing_with_discovered(discovered: HashSet<String>) -> Result<PricingResponse, String> {
    let overrides = load_overrides();
    let catalog = load_catalog();
    let hidden: HashSet<&str> = overrides
        .hidden_model_ids
        .iter()
        .map(String::as_str)
        .collect();
    let mut model_ids: HashSet<String> = builtin_models()
        .into_iter()
        .map(|model| model.model_id.to_string())
        .collect();
    model_ids.extend(catalog.models.iter().map(|model| model.model_id.clone()));
    model_ids.extend(overrides.overrides.keys().cloned());
    model_ids.extend(discovered);

    let mut models: Vec<ModelPricingEntry> = model_ids
        .into_iter()
        .filter(|model_id| !hidden.contains(model_id.as_str()))
        .map(|model_id| {
            let is_custom = overrides.overrides.contains_key(&model_id);
            let effective = effective_rate(&model_id);
            let is_unpriced = effective.is_none();
            let rate = effective.unwrap_or_else(zero_rate);
            let display_name = overrides
                .overrides
                .get(&model_id)
                .and_then(|entry| entry.display_name.clone())
                .or_else(|| catalog_display_name(&model_id, &catalog))
                .or_else(|| builtin_display_name(&model_id).map(str::to_string))
                .unwrap_or_else(|| model_id.clone());
            ModelPricingEntry {
                model_id,
                display_name,
                rate,
                is_custom,
                is_unpriced,
            }
        })
        .collect();
    models.sort_by(|a, b| {
        b.is_unpriced
            .cmp(&a.is_unpriced)
            .then_with(|| a.display_name.cmp(&b.display_name))
    });

    let last_refresh_error = LAST_REFRESH_ERROR
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone();

    Ok(PricingResponse {
        models,
        catalog_fetched_at: catalog.fetched_at,
        last_refresh_error,
    })
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetModelRateRequest {
    pub model_id: String,
    pub display_name: Option<String>,
    pub rate: ModelRate,
}

pub fn set_model_rate(request: SetModelRateRequest) -> Result<PricingResponse, String> {
    let mut overrides = load_overrides();
    overrides
        .hidden_model_ids
        .retain(|id| id != &request.model_id);
    overrides.overrides.insert(
        request.model_id,
        ModelRateOverride {
            display_name: request.display_name,
            rate: request.rate,
        },
    );
    save_overrides(&overrides)?;
    get_pricing()
}

pub fn reset_model_rate(model_id: String) -> Result<PricingResponse, String> {
    let mut overrides = load_overrides();
    overrides.overrides.remove(&model_id);
    save_overrides(&overrides)?;
    get_pricing()
}

pub fn hide_model(model_id: String) -> Result<PricingResponse, String> {
    let mut overrides = load_overrides();
    overrides.overrides.remove(&model_id);
    if !overrides.hidden_model_ids.iter().any(|id| id == &model_id) {
        overrides.hidden_model_ids.push(model_id);
    }
    save_overrides(&overrides)?;
    get_pricing()
}

// Maintainers: keep hosted file in sync with pricing-catalog.example.json
pub const PRICING_CATALOG_URL: &str =
    "https://raw.githubusercontent.com/sheepbooy/Atoll/main/pricing-catalog.example.json";

pub struct FetchResult {
    pub status: u16,
    pub body: Option<String>,
    pub etag: Option<String>,
}

pub fn catalog_is_stale(now_secs: i64, fetched_at: Option<&str>) -> bool {
    let Some(fetched_at) = fetched_at else {
        return true;
    };
    let Ok(parsed) = chrono::DateTime::parse_from_rfc3339(fetched_at) else {
        return true;
    };
    now_secs.saturating_sub(parsed.timestamp()) >= CATALOG_STALE_SECS as i64
}

pub fn apply_fetched_catalog(
    body: &str,
    etag: Option<String>,
    fetched_at: String,
) -> Result<PricingCatalogFile, String> {
    let mut file: PricingCatalogFile =
        serde_json::from_str(body).map_err(|error| format!("invalid catalog JSON: {error}"))?;
    validate_catalog(&file)?;
    file.fetched_at = Some(fetched_at);
    file.etag = etag;
    Ok(file)
}

fn set_last_refresh_error(message: String) {
    *LAST_REFRESH_ERROR
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(message);
}

fn clear_last_refresh_error() {
    *LAST_REFRESH_ERROR
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = None;
}

fn utc_now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

pub fn refresh_pricing_catalog_with_fetcher<F>(
    force: bool,
    fetch: F,
) -> Result<PricingResponse, String>
where
    F: FnOnce(Option<&str>) -> Result<FetchResult, String>,
{
    let catalog = load_catalog();
    if !force {
        if let Some(ref fetched_at) = catalog.fetched_at {
            if !catalog_is_stale(chrono::Utc::now().timestamp(), Some(fetched_at)) {
                return get_pricing();
            }
        }
    }

    let result = match fetch(catalog.etag.as_deref()) {
        Ok(result) => result,
        Err(error) => {
            set_last_refresh_error(error.clone());
            return Err(error);
        }
    };

    match result.status {
        304 => {
            let mut updated = catalog;
            updated.fetched_at = Some(utc_now_rfc3339());
            if let Some(etag) = result.etag {
                updated.etag = Some(etag);
            }
            save_catalog(&updated)?;
            clear_last_refresh_error();
            get_pricing()
        }
        200 => {
            let body = result
                .body
                .ok_or_else(|| "empty catalog response body".to_string())?;
            let new_catalog = match apply_fetched_catalog(&body, result.etag, utc_now_rfc3339()) {
                Ok(catalog) => catalog,
                Err(error) => {
                    set_last_refresh_error(error.clone());
                    return Err(error);
                }
            };
            save_catalog(&new_catalog)?;
            clear_last_refresh_error();
            get_pricing()
        }
        status => {
            let error = format!("unexpected catalog HTTP status {status}");
            set_last_refresh_error(error.clone());
            Err(error)
        }
    }
}

pub fn refresh_pricing_catalog(force: bool) -> Result<PricingResponse, String> {
    refresh_pricing_catalog_with_fetcher(force, |etag| {
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .map_err(|error| error.to_string())?;
        let mut request = client.get(PRICING_CATALOG_URL);
        if let Some(etag) = etag {
            request = request.header(reqwest::header::IF_NONE_MATCH, etag);
        }
        let response = request.send().map_err(|error| error.to_string())?;
        let status = response.status().as_u16();
        let etag = response
            .headers()
            .get(reqwest::header::ETAG)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);
        let body = if status == 200 {
            Some(response.text().map_err(|error| error.to_string())?)
        } else {
            None
        };
        Ok(FetchResult { status, body, etag })
    })
}

pub fn maybe_refresh_pricing_catalog_on_startup() {
    let catalog = load_catalog();
    let now = chrono::Utc::now().timestamp();
    let needs_refresh = catalog
        .fetched_at
        .as_deref()
        .map(|fetched_at| catalog_is_stale(now, Some(fetched_at)))
        .unwrap_or(true);
    if !needs_refresh {
        return;
    }
    if let Err(error) = refresh_pricing_catalog(false) {
        set_last_refresh_error(error);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usage_cost_includes_all_dimensions() {
        let usage = TokenUsage {
            input_tokens: 1_000_000,
            output_tokens: 500_000,
            cache_read_tokens: 200_000,
            cache_creation_tokens: 100_000,
        };
        let rate = ModelRate {
            input_per_million: 3.0,
            output_per_million: 15.0,
            cache_read_per_million: 0.30,
            cache_write_per_million: 3.75,
        };
        let cost = usage_cost_usd(usage, rate);
        assert!((cost - (3.0 + 7.5 + 0.06 + 0.375)).abs() < 0.0001);
    }

    #[test]
    fn unknown_model_is_excluded_from_cost() {
        let mut by_model = HashMap::new();
        by_model.insert(
            UNKNOWN_MODEL.to_string(),
            TokenUsage {
                input_tokens: 1_000_000,
                output_tokens: 0,
                cache_read_tokens: 0,
                cache_creation_tokens: 0,
            },
        );
        assert_eq!(by_model_cost_usd(&by_model), 0.0);
    }

    fn pricing_test_lock() -> std::sync::MutexGuard<'static, ()> {
        crate::PRICING_ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn temp_pricing_paths(test_name: &str) -> (PathBuf, PathBuf) {
        let pid = std::process::id();
        let dir = std::env::temp_dir().join(format!("atoll-pricing-{pid}-{test_name}"));
        let _ = std::fs::create_dir_all(&dir);
        let pricing_path = dir.join("pricing.json");
        let catalog_path = dir.join("pricing-catalog.json");
        (pricing_path, catalog_path)
    }

    fn cleanup_pricing_paths(pricing_path: &PathBuf, catalog_path: &PathBuf) {
        let _ = std::fs::remove_file(pricing_path);
        let _ = std::fs::remove_file(catalog_path);
        if let Some(dir) = pricing_path.parent() {
            let _ = std::fs::remove_dir(dir);
        }
        std::env::remove_var("ATOLL_PRICING_PATH");
        std::env::remove_var("ATOLL_PRICING_CATALOG_PATH");
    }

    #[test]
    fn validate_catalog_rejects_bad_version() {
        let file = PricingCatalogFile {
            version: 99,
            updated_at: None,
            fetched_at: None,
            etag: None,
            models: vec![],
        };
        assert!(validate_catalog(&file).is_err());
    }

    #[test]
    fn effective_rate_prefers_custom_over_catalog_over_builtin() {
        let _lock = pricing_test_lock();
        let (pricing_path, catalog_path) = temp_pricing_paths("effective-rate-priority");
        cleanup_pricing_paths(&pricing_path, &catalog_path);

        std::env::set_var(
            "ATOLL_PRICING_PATH",
            pricing_path.to_string_lossy().as_ref(),
        );
        std::env::set_var(
            "ATOLL_PRICING_CATALOG_PATH",
            catalog_path.to_string_lossy().as_ref(),
        );

        let catalog = PricingCatalogFile {
            version: CATALOG_SCHEMA_VERSION,
            updated_at: None,
            fetched_at: None,
            etag: None,
            models: vec![CatalogModel {
                model_id: "m1".to_string(),
                display_name: "M1".to_string(),
                rate: ModelRate {
                    input_per_million: 1.0,
                    output_per_million: 1.0,
                    cache_read_per_million: 1.0,
                    cache_write_per_million: 1.0,
                },
            }],
        };
        save_catalog(&catalog).expect("save catalog");

        let mut overrides = PricingOverridesFile::default();
        overrides.overrides.insert(
            "m1".to_string(),
            ModelRateOverride {
                display_name: None,
                rate: ModelRate {
                    input_per_million: 9.0,
                    output_per_million: 9.0,
                    cache_read_per_million: 9.0,
                    cache_write_per_million: 9.0,
                },
            },
        );
        save_overrides(&overrides).expect("save overrides");

        assert_eq!(effective_rate("m1").unwrap().input_per_million, 9.0);

        overrides.overrides.remove("m1");
        save_overrides(&overrides).expect("clear overrides");
        assert_eq!(effective_rate("m1").unwrap().input_per_million, 1.0);

        let empty_catalog = PricingCatalogFile {
            version: CATALOG_SCHEMA_VERSION,
            updated_at: None,
            fetched_at: None,
            etag: None,
            models: vec![],
        };
        save_catalog(&empty_catalog).expect("clear catalog");

        let builtin = effective_rate("gpt-4o").expect("builtin rate");
        assert_eq!(builtin.input_per_million, 2.50);

        cleanup_pricing_paths(&pricing_path, &catalog_path);
    }

    #[test]
    fn get_pricing_includes_unpriced_discovered_models() {
        let _lock = pricing_test_lock();
        let (pricing_path, catalog_path) = temp_pricing_paths("unpriced-discovered");
        cleanup_pricing_paths(&pricing_path, &catalog_path);
        std::env::set_var(
            "ATOLL_PRICING_PATH",
            pricing_path.to_string_lossy().as_ref(),
        );
        std::env::set_var(
            "ATOLL_PRICING_CATALOG_PATH",
            catalog_path.to_string_lossy().as_ref(),
        );

        let response =
            get_pricing_with_discovered(HashSet::from(["my-real-model".into()])).unwrap();
        let entry = response
            .models
            .iter()
            .find(|m| m.model_id == "my-real-model")
            .unwrap();
        assert!(entry.is_unpriced);
        assert!(!entry.is_custom);

        cleanup_pricing_paths(&pricing_path, &catalog_path);
    }

    #[test]
    fn get_pricing_sorts_unpriced_first() {
        let _lock = pricing_test_lock();
        let (pricing_path, catalog_path) = temp_pricing_paths("sort-unpriced-first");
        cleanup_pricing_paths(&pricing_path, &catalog_path);
        std::env::set_var(
            "ATOLL_PRICING_PATH",
            pricing_path.to_string_lossy().as_ref(),
        );
        std::env::set_var(
            "ATOLL_PRICING_CATALOG_PATH",
            catalog_path.to_string_lossy().as_ref(),
        );

        let response =
            get_pricing_with_discovered(HashSet::from(["zzz-unpriced".into()])).unwrap();
        assert_eq!(response.models[0].model_id, "zzz-unpriced");
        assert!(response.models[0].is_unpriced);

        cleanup_pricing_paths(&pricing_path, &catalog_path);
    }

    #[test]
    fn catalog_is_stale_when_missing_or_old() {
        assert!(catalog_is_stale(1_700_000_000, None));
        assert!(catalog_is_stale(1_700_000_000, Some("2000-01-01T00:00:00Z")));
    }

    #[test]
    fn apply_fetched_catalog_rejects_invalid_json() {
        assert!(apply_fetched_catalog("{nope}", None, "2026-07-09T00:00:00Z".into()).is_err());
    }

    #[test]
    fn refresh_with_fetcher_304_bumps_fetched_at_only() {
        let _lock = pricing_test_lock();
        let (pricing_path, catalog_path) = temp_pricing_paths("refresh-304");
        cleanup_pricing_paths(&pricing_path, &catalog_path);
        std::env::set_var(
            "ATOLL_PRICING_PATH",
            pricing_path.to_string_lossy().as_ref(),
        );
        std::env::set_var(
            "ATOLL_PRICING_CATALOG_PATH",
            catalog_path.to_string_lossy().as_ref(),
        );

        let old_fetched = "2000-01-01T00:00:00Z".to_string();
        let models = vec![CatalogModel {
            model_id: "test-model".to_string(),
            display_name: "Test Model".to_string(),
            rate: ModelRate {
                input_per_million: 1.0,
                output_per_million: 2.0,
                cache_read_per_million: 0.1,
                cache_write_per_million: 0.2,
            },
        }];
        let catalog = PricingCatalogFile {
            version: CATALOG_SCHEMA_VERSION,
            updated_at: Some("2026-07-09T00:00:00Z".into()),
            fetched_at: Some(old_fetched.clone()),
            etag: Some("W/\"abc\"".into()),
            models: models.clone(),
        };
        save_catalog(&catalog).expect("seed catalog");

        refresh_pricing_catalog_with_fetcher(true, |_etag| {
            Ok(FetchResult {
                status: 304,
                body: None,
                etag: Some("W/\"abc\"".into()),
            })
        })
        .expect("304 refresh");

        let reloaded = load_catalog();
        assert_ne!(reloaded.fetched_at.as_ref().unwrap(), &old_fetched);
        assert_eq!(reloaded.models, models);

        cleanup_pricing_paths(&pricing_path, &catalog_path);
    }

    #[test]
    fn hide_model_filters_settings_list_but_keeps_effective_rate() {
        let _lock = pricing_test_lock();
        let (pricing_path, catalog_path) = temp_pricing_paths("hide-model");
        cleanup_pricing_paths(&pricing_path, &catalog_path);
        std::env::set_var(
            "ATOLL_PRICING_PATH",
            pricing_path.to_string_lossy().as_ref(),
        );
        std::env::set_var(
            "ATOLL_PRICING_CATALOG_PATH",
            catalog_path.to_string_lossy().as_ref(),
        );

        let mut overrides = PricingOverridesFile::default();
        overrides.overrides.insert(
            "custom-only".to_string(),
            ModelRateOverride {
                display_name: Some("Custom Only".into()),
                rate: ModelRate {
                    input_per_million: 7.0,
                    output_per_million: 7.0,
                    cache_read_per_million: 7.0,
                    cache_write_per_million: 7.0,
                },
            },
        );
        save_overrides(&overrides).expect("seed overrides");

        hide_model("gpt-4o".into()).expect("hide builtin");
        hide_model("custom-only".into()).expect("hide custom");

        let response = get_pricing_with_discovered(HashSet::new()).unwrap();
        assert!(
            !response
                .models
                .iter()
                .any(|model| model.model_id == "gpt-4o" || model.model_id == "custom-only")
        );
        assert_eq!(effective_rate("gpt-4o").unwrap().input_per_million, 2.50);
        assert_eq!(effective_rate("custom-only"), None);

        let reloaded = load_overrides();
        assert!(reloaded.hidden_model_ids.contains(&"gpt-4o".to_string()));
        assert!(reloaded.hidden_model_ids.contains(&"custom-only".to_string()));
        assert!(!reloaded.overrides.contains_key("custom-only"));

        cleanup_pricing_paths(&pricing_path, &catalog_path);
    }

    #[test]
    fn hide_model_filters_discovered_unpriced_models() {
        let _lock = pricing_test_lock();
        let (pricing_path, catalog_path) = temp_pricing_paths("hide-discovered");
        cleanup_pricing_paths(&pricing_path, &catalog_path);
        std::env::set_var(
            "ATOLL_PRICING_PATH",
            pricing_path.to_string_lossy().as_ref(),
        );
        std::env::set_var(
            "ATOLL_PRICING_CATALOG_PATH",
            catalog_path.to_string_lossy().as_ref(),
        );

        hide_model("discovered-model".into()).expect("hide discovered");

        let response = get_pricing_with_discovered(HashSet::from(["discovered-model".into()]))
            .unwrap();
        assert!(
            !response
                .models
                .iter()
                .any(|model| model.model_id == "discovered-model")
        );

        cleanup_pricing_paths(&pricing_path, &catalog_path);
    }

    #[test]
    fn set_model_rate_unhides_model() {
        let _lock = pricing_test_lock();
        let (pricing_path, catalog_path) = temp_pricing_paths("unhide-on-set");
        cleanup_pricing_paths(&pricing_path, &catalog_path);
        std::env::set_var(
            "ATOLL_PRICING_PATH",
            pricing_path.to_string_lossy().as_ref(),
        );
        std::env::set_var(
            "ATOLL_PRICING_CATALOG_PATH",
            catalog_path.to_string_lossy().as_ref(),
        );

        hide_model("gpt-4o".into()).expect("hide builtin");

        set_model_rate(SetModelRateRequest {
            model_id: "gpt-4o".into(),
            display_name: None,
            rate: ModelRate {
                input_per_million: 4.0,
                output_per_million: 4.0,
                cache_read_per_million: 4.0,
                cache_write_per_million: 4.0,
            },
        })
        .expect("set rate");

        let response = get_pricing_with_discovered(HashSet::new()).unwrap();
        let entry = response
            .models
            .iter()
            .find(|model| model.model_id == "gpt-4o")
            .expect("model visible again");
        assert!(entry.is_custom);
        assert_eq!(entry.rate.input_per_million, 4.0);

        cleanup_pricing_paths(&pricing_path, &catalog_path);
    }

    #[test]
    fn refresh_with_fetcher_200_replaces_models() {
        let _lock = pricing_test_lock();
        let (pricing_path, catalog_path) = temp_pricing_paths("refresh-200");
        cleanup_pricing_paths(&pricing_path, &catalog_path);
        std::env::set_var(
            "ATOLL_PRICING_PATH",
            pricing_path.to_string_lossy().as_ref(),
        );
        std::env::set_var(
            "ATOLL_PRICING_CATALOG_PATH",
            catalog_path.to_string_lossy().as_ref(),
        );

        let catalog = PricingCatalogFile {
            version: CATALOG_SCHEMA_VERSION,
            updated_at: Some("2026-01-01T00:00:00Z".into()),
            fetched_at: Some("2026-01-01T00:00:00Z".into()),
            etag: Some("W/\"old\"".into()),
            models: vec![CatalogModel {
                model_id: "old-model".to_string(),
                display_name: "Old".to_string(),
                rate: ModelRate {
                    input_per_million: 1.0,
                    output_per_million: 1.0,
                    cache_read_per_million: 1.0,
                    cache_write_per_million: 1.0,
                },
            }],
        };
        save_catalog(&catalog).expect("seed catalog");

        let new_body = r#"{
            "version": 1,
            "updatedAt": "2026-07-09T00:00:00Z",
            "models": [{
                "modelId": "new-model",
                "displayName": "New Model",
                "rate": {
                    "inputPerMillion": 5.0,
                    "outputPerMillion": 10.0,
                    "cacheReadPerMillion": 0.5,
                    "cacheWritePerMillion": 1.0
                }
            }]
        }"#;

        refresh_pricing_catalog_with_fetcher(true, |_etag| {
            Ok(FetchResult {
                status: 200,
                body: Some(new_body.to_string()),
                etag: Some("W/\"new\"".into()),
            })
        })
        .expect("200 refresh");

        let reloaded = load_catalog();
        assert_eq!(reloaded.models.len(), 1);
        assert_eq!(reloaded.models[0].model_id, "new-model");
        assert_eq!(reloaded.etag.as_deref(), Some("W/\"new\""));
        assert!(reloaded.fetched_at.is_some());

        cleanup_pricing_paths(&pricing_path, &catalog_path);
    }
}
