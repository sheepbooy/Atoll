import { useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { ClipboardList, Search, Trash2 } from "lucide-react";
import i18n from "./i18n";
import type { ClipboardEntry } from "./tauri";

interface ClipboardHistoryViewProps {
  entries: ClipboardEntry[];
  enabled: boolean;
  onCopy: (id: string) => void;
  onClear: () => void;
}

function timeAgoFromSecs(unixSecs: number) {
  const elapsed = Math.max(1, Math.floor(Date.now() / 1000 - unixSecs));
  if (elapsed < 60) {
    return i18n.t("time.agoSeconds", { seconds: elapsed });
  }
  const minutes = Math.floor(elapsed / 60);
  if (minutes < 60) {
    return i18n.t("time.agoMinutes", { minutes });
  }
  const hours = Math.floor(minutes / 60);
  return i18n.t("time.agoHours", { hours });
}

export function ClipboardHistoryView({
  entries,
  enabled,
  onCopy,
  onClear,
}: ClipboardHistoryViewProps) {
  const { t } = useTranslation();
  const [search, setSearch] = useState("");
  const [copiedId, setCopiedId] = useState<string | null>(null);

  const filtered = useMemo(() => {
    if (!search.trim()) return entries;
    const q = search.toLowerCase();
    return entries.filter((e) => e.preview.toLowerCase().includes(q));
  }, [entries, search]);

  const handleCopy = (id: string) => {
    onCopy(id);
    setCopiedId(id);
    window.setTimeout(() => setCopiedId(null), 1200);
  };

  return (
    <div className="clipboard-history-view settings-view" data-no-drag>
      <div className="settings-body">
        {!enabled ? (
          <div className="clipboard-empty">
            <div className="clipboard-empty-icon">
              <ClipboardList size={24} />
            </div>
            <p>{t("clipboard.disabled")}</p>
          </div>
        ) : (
          <>
            <div className="clipboard-toolbar">
              <div className="clipboard-search-wrap">
                <Search size={13} className="clipboard-search-icon" />
                <input
                  type="text"
                  className="clipboard-search-input"
                  placeholder={t("clipboard.searchPlaceholder")}
                  value={search}
                  onChange={(e) => setSearch(e.target.value)}
                />
              </div>
              {entries.length > 0 ? (
                <button
                  type="button"
                  className="clipboard-clear-btn"
                  onClick={onClear}
                  data-no-drag
                >
                  <Trash2 size={13} />
                  <span>{t("clipboard.clear")}</span>
                </button>
              ) : null}
            </div>

            {filtered.length === 0 ? (
              <div className="clipboard-empty">
                <div className="clipboard-empty-icon">
                  <ClipboardList size={24} />
                </div>
                <p>{t("clipboard.empty")}</p>
              </div>
            ) : (
              <div className="clipboard-list">
                {filtered.map((entry) => (
                  <button
                    key={entry.id}
                    type="button"
                    className={`clipboard-entry${copiedId === entry.id ? " is-copied" : ""}`}
                    onClick={() => handleCopy(entry.id)}
                    data-no-drag
                    title={entry.content}
                  >
                    <span className="clipboard-entry-preview">
                      {entry.preview}
                    </span>
                    <span className="clipboard-entry-meta">
                      {copiedId === entry.id
                        ? t("clipboard.copied")
                        : timeAgoFromSecs(entry.copiedAt)}
                    </span>
                  </button>
                ))}
              </div>
            )}
          </>
        )}
      </div>
    </div>
  );
}
