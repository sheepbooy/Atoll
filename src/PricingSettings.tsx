import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import type { ModelPricingEntry, ModelRate, PricingResponse } from "./pricing";
import {
  getPricing,
  hideModel,
  refreshPricing,
  resetModelRate,
  setModelRate,
  unhideModel,
} from "./pricing";

interface PricingSettingsProps {
  models: ModelPricingEntry[];
  onModelsChange: (models: ModelPricingEntry[]) => void;
}

function formatCatalogAge(iso: string, t: (key: string, options?: Record<string, unknown>) => string): string {
  const parsed = Date.parse(iso);
  if (!Number.isFinite(parsed)) return iso;
  const ageMs = Date.now() - parsed;
  if (ageMs < 60_000) return t("pricing.justNow");
  const hours = Math.floor(ageMs / 3_600_000);
  if (hours < 24) return t("pricing.hoursAgo", { hours });
  const days = Math.floor(hours / 24);
  return t("pricing.daysAgo", { days });
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

function PricingModelEditor({
  model,
  draftRate,
  busy,
  onDraftChange,
  onCancel,
  onSave,
}: {
  model: ModelPricingEntry;
  draftRate: ModelRate;
  busy: boolean;
  onDraftChange: (rate: ModelRate) => void;
  onCancel: () => void;
  onSave: () => void;
}) {
  const { t } = useTranslation("tokens");
  const editorRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    editorRef.current?.scrollIntoView({ block: "nearest", behavior: "smooth" });
  }, [model.modelId]);

  return (
    <div className="pricing-editor" ref={editorRef}>
      <span className="settings-section-label">
        {t("pricing.editModel", { name: model.displayName })}
      </span>
      <div className="pricing-rate-grid">
        <RateField
          label={t("pricing.inputPerM")}
          value={draftRate.inputPerMillion}
          onChange={(value) =>
            onDraftChange({ ...draftRate, inputPerMillion: value })
          }
        />
        <RateField
          label={t("pricing.outputPerM")}
          value={draftRate.outputPerMillion}
          onChange={(value) =>
            onDraftChange({ ...draftRate, outputPerMillion: value })
          }
        />
        <RateField
          label={t("pricing.cacheReadPerM")}
          value={draftRate.cacheReadPerMillion}
          onChange={(value) =>
            onDraftChange({ ...draftRate, cacheReadPerMillion: value })
          }
        />
        <RateField
          label={t("pricing.cacheWritePerM")}
          value={draftRate.cacheWritePerMillion}
          onChange={(value) =>
            onDraftChange({ ...draftRate, cacheWritePerMillion: value })
          }
        />
      </div>
      <div className="pricing-editor-actions">
        <button
          type="button"
          className="settings-inline-button"
          disabled={busy}
          onClick={onCancel}
          data-no-drag
        >
          {t("pricing.cancel")}
        </button>
        <button
          type="button"
          className="settings-inline-button is-primary"
          disabled={busy}
          onClick={onSave}
          data-no-drag
        >
          {t("pricing.save")}
        </button>
      </div>
    </div>
  );
}

export function PricingSettings({ models, onModelsChange }: PricingSettingsProps) {
  const { t } = useTranslation("tokens");
  const [editingId, setEditingId] = useState<string | null>(null);
  const [draftRate, setDraftRate] = useState<ModelRate | null>(null);
  const [busy, setBusy] = useState(false);
  const [refreshing, setRefreshing] = useState(false);
  const [catalogFetchedAt, setCatalogFetchedAt] = useState<string | null>(null);
  const [lastRefreshError, setLastRefreshError] = useState<string | null>(null);
  const [hiddenModels, setHiddenModels] = useState<ModelPricingEntry[]>([]);

  function applyResponse(response: PricingResponse) {
    onModelsChange(response.models);
    setHiddenModels(response.hiddenModels ?? []);
    setCatalogFetchedAt(response.catalogFetchedAt);
    setLastRefreshError(response.lastRefreshError);
  }

  useEffect(() => {
    getPricing()
      .then((response) => {
        setHiddenModels(response.hiddenModels ?? []);
        setCatalogFetchedAt(response.catalogFetchedAt);
        setLastRefreshError(response.lastRefreshError);
      })
      .catch(() => undefined);
  }, []);

  async function refreshModels() {
    const response = await getPricing();
    applyResponse(response);
  }

  async function handleRefreshCatalog() {
    setRefreshing(true);
    try {
      const response = await refreshPricing();
      applyResponse(response);
    } catch {
      setLastRefreshError(t("update.refreshFailed", { ns: "errors" }));
    } finally {
      setRefreshing(false);
    }
  }

  async function handleSave(model: ModelPricingEntry) {
    if (!draftRate) return;
    setBusy(true);
    try {
      const response = await setModelRate({
        modelId: model.modelId,
        displayName: model.displayName,
        rate: draftRate,
      });
      applyResponse(response);
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
      applyResponse(response);
      if (editingId === modelId) {
        setEditingId(null);
        setDraftRate(null);
      }
    } finally {
      setBusy(false);
    }
  }

  async function handleDelete(modelId: string) {
    setBusy(true);
    try {
      const response = await hideModel(modelId);
      applyResponse(response);
      if (editingId === modelId) {
        setEditingId(null);
        setDraftRate(null);
      }
    } finally {
      setBusy(false);
    }
  }

  async function handleRestore(modelId: string) {
    setBusy(true);
    try {
      const response = await unhideModel(modelId);
      applyResponse(response);
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="pricing-settings">
      <div className="pricing-refresh-row">
        <span className="settings-card-desc">
          {lastRefreshError
            ? t("pricing.refreshFailed")
            : catalogFetchedAt
              ? t("pricing.lastRefreshed", {
                  age: formatCatalogAge(catalogFetchedAt, t),
                })
              : t("pricing.neverRefreshed")}
        </span>
        <button
          type="button"
          className="settings-inline-button"
          disabled={busy || refreshing}
          onClick={handleRefreshCatalog}
          data-no-drag
        >
          {refreshing ? t("pricing.refreshing") : t("pricing.refreshPricing")}
        </button>
      </div>

      <p className="settings-card-desc pricing-settings-note">
        {t("pricing.note")}
      </p>

      <div className="pricing-model-list">
        {models.map((model) => {
          const isEditing = editingId === model.modelId;
          return (
            <div
              key={model.modelId}
              className={`pricing-model-row${isEditing ? " is-editing" : ""}`}
            >
              <div className="pricing-model-row-main">
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
                    {model.isUnpriced
                      ? t("pricing.unpriced")
                      : model.isCustom
                        ? t("pricing.custom")
                        : t("pricing.default")}
                  </span>
                  {isEditing ? null : (
                    <>
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
                        {t("pricing.edit")}
                      </button>
                      {model.isCustom ? (
                        <button
                          type="button"
                          className="settings-inline-button"
                          disabled={busy}
                          onClick={() => handleReset(model.modelId)}
                          data-no-drag
                        >
                          {t("pricing.reset")}
                        </button>
                      ) : null}
                      <button
                        type="button"
                        className="settings-inline-button is-danger"
                        disabled={busy}
                        onClick={() => handleDelete(model.modelId)}
                        data-no-drag
                      >
                        {t("pricing.delete")}
                      </button>
                    </>
                  )}
                </div>
              </div>
              {isEditing && draftRate ? (
                <PricingModelEditor
                  model={model}
                  draftRate={draftRate}
                  busy={busy}
                  onDraftChange={setDraftRate}
                  onCancel={() => {
                    setEditingId(null);
                    setDraftRate(null);
                  }}
                  onSave={() => handleSave(model)}
                />
              ) : null}
            </div>
          );
        })}
      </div>

      {hiddenModels.length > 0 ? (
        <div className="pricing-hidden-section">
          <span className="settings-section-label">{t("pricing.hiddenModels")}</span>
          <p className="settings-card-desc pricing-settings-note">
            {t("pricing.hiddenModelsDesc")}
          </p>
          <div className="pricing-model-list">
            {hiddenModels.map((model) => (
              <div key={model.modelId} className="pricing-model-row is-hidden">
                <div className="pricing-model-row-main">
                  <div className="pricing-model-copy">
                    <span className="settings-card-title">{model.displayName}</span>
                    <span className="settings-card-desc">{model.modelId}</span>
                  </div>
                  <div className="pricing-model-meta">
                    <span className="settings-hook-badge is-summary is-hidden">
                      {t("pricing.hidden")}
                    </span>
                    <button
                      type="button"
                      className="settings-inline-button"
                      disabled={busy}
                      onClick={() => handleRestore(model.modelId)}
                      data-no-drag
                    >
                      {t("pricing.restore")}
                    </button>
                  </div>
                </div>
              </div>
            ))}
          </div>
        </div>
      ) : null}

      {models.length === 0 && hiddenModels.length === 0 ? (
        <button
          type="button"
          className="settings-inline-button"
          disabled={busy}
          onClick={() => refreshModels()}
          data-no-drag
        >
          {t("pricing.loadPricing")}
        </button>
      ) : null}
    </div>
  );
}
