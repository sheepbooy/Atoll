import { invoke } from "@tauri-apps/api/core";
import type { TokenUsage } from "./tauri";

function isTauriRuntimeForPricing(): boolean {
  return "__TAURI_INTERNALS__" in window;
}

export async function getPricing(): Promise<PricingResponse> {
  if (isTauriRuntimeForPricing()) {
    return invoke<PricingResponse>("get_pricing");
  }
  return { models: [], catalogFetchedAt: null, lastRefreshError: null };
}

export async function setModelRate(request: SetModelRateRequest): Promise<PricingResponse> {
  if (isTauriRuntimeForPricing()) {
    return invoke<PricingResponse>("set_model_rate", { request });
  }
  return { models: [], catalogFetchedAt: null, lastRefreshError: null };
}

export async function resetModelRate(modelId: string): Promise<PricingResponse> {
  if (isTauriRuntimeForPricing()) {
    return invoke<PricingResponse>("reset_model_rate", { modelId });
  }
  return { models: [], catalogFetchedAt: null, lastRefreshError: null };
}

export async function hideModel(modelId: string): Promise<PricingResponse> {
  if (isTauriRuntimeForPricing()) {
    return invoke<PricingResponse>("hide_model", { modelId });
  }
  return { models: [], catalogFetchedAt: null, lastRefreshError: null };
}

export async function refreshPricing(): Promise<PricingResponse> {
  if (isTauriRuntimeForPricing()) {
    return invoke<PricingResponse>("refresh_pricing");
  }
  return { models: [], catalogFetchedAt: null, lastRefreshError: null };
}

export type { ModelPricingEntry, ModelRate, PricingResponse, SetModelRateRequest };

export interface SetModelRateRequest {
  modelId: string;
  displayName?: string;
  rate: ModelRate;
}


export interface ModelRate {
  inputPerMillion: number;
  outputPerMillion: number;
  cacheReadPerMillion: number;
  cacheWritePerMillion: number;
}

export interface ModelPricingEntry {
  modelId: string;
  displayName: string;
  rate: ModelRate;
  isCustom: boolean;
  isUnpriced: boolean;
}

export interface PricingResponse {
  models: ModelPricingEntry[];
  catalogFetchedAt: string | null;
  lastRefreshError: string | null;
}

export const UNKNOWN_MODEL = "_unknown";

export function usageCostUsd(usage: TokenUsage, rate: ModelRate): number {
  return (
    (usage.inputTokens * rate.inputPerMillion +
      usage.outputTokens * rate.outputPerMillion +
      usage.cacheReadTokens * rate.cacheReadPerMillion +
      usage.cacheCreationTokens * rate.cacheWritePerMillion) /
    1_000_000
  );
}

export function pricingRateMap(models: ModelPricingEntry[]): Record<string, ModelRate> {
  return Object.fromEntries(
    models.filter((model) => !model.isUnpriced).map((model) => [model.modelId, model.rate]),
  );
}

export function byModelCostUsd(
  byModel: Record<string, TokenUsage> | undefined,
  rates: Record<string, ModelRate>,
): number {
  if (!byModel) return 0;
  let total = 0;
  for (const [modelId, usage] of Object.entries(byModel)) {
    if (modelId === UNKNOWN_MODEL) continue;
    const rate = rates[modelId];
    if (!rate) continue;
    total += usageCostUsd(usage, rate);
  }
  return total;
}
