import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { ClipboardList, Image as ImageIcon, Paperclip, Search, Trash2 } from "lucide-react";
import i18n from "./i18n";
import { getClipboardEntryThumbnail } from "./tauri";
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

function formatBytes(bytes: number) {
  if (!bytes) {
    return "";
  }
  if (bytes < 1024) {
    return `${bytes} B`;
  }
  if (bytes < 1024 * 1024) {
    return `${Math.max(1, Math.round(bytes / 1024))} KB`;
  }
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}

function entryTooltip(entry: ClipboardEntry) {
  if (entry.kind === "image") {
    return i18n.t("clipboard.kindImage");
  }
  if (entry.kind === "files") {
    return entry.content;
  }
  return entry.content;
}

function ClipboardEntryRow({
  entry,
  copied,
  onCopy,
}: {
  entry: ClipboardEntry;
  copied: boolean;
  onCopy: (id: string) => void;
}) {
  const { t } = useTranslation();
  const [thumb, setThumb] = useState<string | null>(null);
  const isImage = entry.kind === "image";
  const isFiles = entry.kind === "files";

  useEffect(() => {
    if (!isImage) {
      return;
    }
    let cancelled = false;
    getClipboardEntryThumbnail(entry.id)
      .then((url) => {
        if (!cancelled) {
          setThumb(url);
        }
      })
      .catch(() => undefined);
    return () => {
      cancelled = true;
    };
  }, [entry.id, isImage]);

  const metaExtras = isImage
    ? formatBytes(entry.byteSize ?? 0)
    : isFiles
      ? t("clipboard.kindFiles")
      : "";
  const withIcon = isImage || isFiles;

  const icon = isImage ? (
    thumb ? (
      <img
        className="clipboard-entry-thumb"
        src={thumb}
        alt={t("clipboard.kindImage")}
        draggable={false}
      />
    ) : (
      <span className="clipboard-entry-icon">
        <ImageIcon size={16} />
      </span>
    )
  ) : isFiles ? (
    <span className="clipboard-entry-icon">
      <Paperclip size={14} />
    </span>
  ) : null;

  const body = (
    <>
      <span className="clipboard-entry-preview">
        {isImage ? t("clipboard.kindImage") : entry.preview}
      </span>
      <span className="clipboard-entry-meta">
        {copied
          ? t("clipboard.copied")
          : [metaExtras, timeAgoFromSecs(entry.copiedAt)]
              .filter(Boolean)
              .join(" · ")}
      </span>
    </>
  );

  return (
    <button
      type="button"
      className={`clipboard-entry${copied ? " is-copied" : ""}${withIcon ? " has-icon" : ""}`}
      onClick={() => onCopy(entry.id)}
      data-no-drag
      title={entryTooltip(entry)}
    >
      {withIcon ? (
        <>
          {icon}
          <span className="clipboard-entry-body">{body}</span>
        </>
      ) : (
        body
      )}
    </button>
  );
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
    return entries.filter((e) => {
      if (e.kind === "image") {
        return t("clipboard.kindImage").toLowerCase().includes(q);
      }
      return e.preview.toLowerCase().includes(q);
    });
  }, [entries, search, t]);

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
                  <ClipboardEntryRow
                    key={entry.id}
                    entry={entry}
                    copied={copiedId === entry.id}
                    onCopy={handleCopy}
                  />
                ))}
              </div>
            )}
          </>
        )}
      </div>
    </div>
  );
}
