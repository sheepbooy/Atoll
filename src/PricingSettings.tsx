import { useEffect, useRef, useState } from "react";
import type { ModelPricingEntry, ModelRate } from "./pricing";
import { getPricing, refreshPricing, resetModelRate, setModelRate } from "./pricing";

interface PricingSettingsProps {
  models: ModelPricingEntry[];
  onModelsChange: (models: ModelPricingEntry[]) => void;
}

function formatCatalogAge(iso: string): string {
  const parsed = Date.parse(iso);
  if (!Number.isFinite(parsed)) return iso;
  const ageMs = Date.now() - parsed;
  if (ageMs < 60_000) return "just now";
  const hours = Math.floor(ageMs / 3_600_000);
  if (hours < 24) return `${hours}h ago`;
  const days = Math.floor(hours / 24);
  return `${days}d ago`;
}

function RateField({
  label,
  value,
  onChange,
}: {
  label: string;
  value: number;
  onChange: (value: number) => void;
}) {
  return (
    <label className="pricing-rate-field">
      <span>{label}</span>
      <input
        type="number"
        min={0}
        step={0.01}
        value={Number.isFinite(value) ? value : 0}
        onChange={(event) => onChange(Number(event.target.value))}
        data-no-drag
      />
    </label>
  );
}

export function PricingSettings({ models, onModelsChange }: PricingSettingsProps) {
  const [editingId, setEditingId] = useState<string | null>(null);
  const [draftRate, setDraftRate] = useState<ModelRate | null>(null);
  const [busy, setBusy] = useState(false);
  const [refreshing, setRefreshing] = useState(false);
  const [catalogFetchedAt, setCatalogFetchedAt] = useState<string | null>(null);
  const [lastRefreshError, setLastRefreshError] = useState<string | null>(null);
  const editorRef = useRef<HTMLDivElement | null>(null);

  const editingModel = models.find((model) => model.modelId === editingId) ?? null;

  useEffect(() => {
    getPricing()
      .then((response) => {
        setCatalogFetchedAt(response.catalogFetchedAt);
        setLastRefreshError(response.lastRefreshError);
      })
      .catch(() => undefined);
  }, []);

  useEffect(() => {
    if (!editingModel || !draftRate || !editorRef.current) return;
    editorRef.current.scrollIntoView({ block: "nearest", behavior: "smooth" });
  }, [editingModel, draftRate]);

  async function refreshModels() {
    const response = await getPricing();
    onModelsChange(response.models);
    setCatalogFetchedAt(response.catalogFetchedAt);
    setLastRefreshError(response.lastRefreshError);
  }

  async function handleRefreshCatalog() {
    setRefreshing(true);
    try {
      const response = await refreshPricing();
      onModelsChange(response.models);
      setCatalogFetchedAt(response.catalogFetchedAt);
      setLastRefreshError(response.lastRefreshError);
    } finally {
      setRefreshing(false);
    }
  }

  async function handleSave() {
    if (!editingModel || !draftRate) return;
    setBusy(true);
    try {
      const response = await setModelRate({
        modelId: editingModel.modelId,
        displayName: editingModel.displayName,
        rate: draftRate,
      });
      onModelsChange(response.models);
      setEditingId(null);
      setDraftRate(null);
    } finally {
      setBusy(false);
    }
  }

  async function handleReset(modelId: string) {
    setBusy(true);
    try {
      const response = await resetModelRate(modelId);
      onModelsChange(response.models);
      if (editingId === modelId) {
        setEditingId(null);
        setDraftRate(null);
      }
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="pricing-settings">
      <div className="pricing-refresh-row">
        <span className="settings-card-desc">
          {lastRefreshError
            ? "Refresh failed"
            : catalogFetchedAt
              ? `Last refreshed · ${formatCatalogAge(catalogFetchedAt)}`
              : "Never refreshed"}
        </span>
        <button
          type="button"
          className="settings-inline-button"
          disabled={busy || refreshing}
          onClick={handleRefreshCatalog}
          data-no-drag
        >
          {refreshing ? "Refreshing…" : "Refresh pricing"}
        </button>
      </div>

      <p className="settings-card-desc pricing-settings-note">
        Only usage with a known model and rate is priced. Older history and usage
        without model metadata are excluded from cost totals.
      </p>

      {editingModel && draftRate ? (
        <div className="pricing-editor" ref={editorRef}>
          <span className="settings-section-label">Edit {editingModel.displayName}</span>
          <div className="pricing-rate-grid">
            <RateField
              label="Input / 1M"
              value={draftRate.inputPerMillion}
              onChange={(value) =>
                setDraftRate((current) =>
                  current ? { ...current, inputPerMillion: value } : current,
                )
              }
            />
            <RateField
              label="Output / 1M"
              value={draftRate.outputPerMillion}
              onChange={(value) =>
                setDraftRate((current) =>
                  current ? { ...current, outputPerMillion: value } : current,
                )
              }
            />
            <RateField
              label="Cache read / 1M"
              value={draftRate.cacheReadPerMillion}
              onChange={(value) =>
                setDraftRate((current) =>
                  current ? { ...current, cacheReadPerMillion: value } : current,
                )
              }
            />
            <RateField
              label="Cache write / 1M"
              value={draftRate.cacheWritePerMillion}
              onChange={(value) =>
                setDraftRate((current) =>
                  current ? { ...current, cacheWritePerMillion: value } : current,
                )
              }
            />
          </div>
          <div className="pricing-editor-actions">
            <button
              type="button"
              className="settings-inline-button"
              disabled={busy}
              onClick={() => {
                setEditingId(null);
                setDraftRate(null);
              }}
              data-no-drag
            >
              Cancel
            </button>
            <button
              type="button"
              className="settings-inline-button is-primary"
              disabled={busy}
              onClick={handleSave}
              data-no-drag
            >
              Save
            </button>
          </div>
        </div>
      ) : null}

      <div className="pricing-model-list">
        {models.map((model) => (
          <div
            key={model.modelId}
            className={`pricing-model-row${
              editingId === model.modelId ? " is-editing" : ""
            }`}
          >
            <div className="pricing-model-copy">
              <span className="settings-card-title">{model.displayName}</span>
              <span className="settings-card-desc">{model.modelId}</span>
            </div>
            <div className="pricing-model-meta">
              <span
                className={`settings-hook-badge is-summary${
                  model.isUnpriced
                    ? " is-unpriced"
                    : model.isCustom
                      ? ""
                      : " is-installed"
                }`}
              >
                {model.isUnpriced ? "Unpriced" : model.isCustom ? "Custom" : "Default"}
              </span>
              <button
                type="button"
                className="settings-inline-button"
                disabled={busy}
                onClick={() => {
                  setEditingId(model.modelId);
                  setDraftRate(model.rate);
                }}
                data-no-drag
              >
                Edit
              </button>
              {model.isCustom ? (
                <button
                  type="button"
                  className="settings-inline-button"
                  disabled={busy}
                  onClick={() => handleReset(model.modelId)}
                  data-no-drag
                >
                  Reset
                </button>
              ) : null}
            </div>
          </div>
        ))}
      </div>

      {models.length === 0 ? (
        <button
          type="button"
          className="settings-inline-button"
          disabled={busy}
          onClick={() => refreshModels()}
          data-no-drag
        >
          Load pricing
        </button>
      ) : null}
    </div>
  );
}
