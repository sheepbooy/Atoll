import { useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Copy, FolderOpen, Inbox, Search, Trash2, X } from "lucide-react";
import i18n from "./i18n";
import { STAGED_FILES_LIMIT, type StagedFile } from "./tauri";

interface FileStationViewProps {
  files: StagedFile[];
  onCopy: (id: string) => void;
  onReveal: (path: string) => void;
  onRemove: (id: string) => void;
  onClear: () => void;
  /** Begin a native drag-out of the given files when the user drags a row away. */
  onDragOut: (ids: string[]) => void;
}

function timeAgoFromMs(ms: number) {
  const elapsed = Math.max(1, Math.floor((Date.now() - ms) / 1000));
  if (elapsed < 60) {
    return i18n.t("time.agoSeconds", { seconds: elapsed });
  }
  const minutes = Math.floor(elapsed / 60);
  if (minutes < 60) {
    return i18n.t("time.agoMinutes", { minutes });
  }
  const hours = Math.floor(minutes / 60);
  if (hours < 24) {
    return i18n.t("time.agoHours", { hours });
  }
  const days = Math.floor(hours / 24);
  return i18n.t("time.agoDays", { days });
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
  if (bytes < 1024 * 1024 * 1024) {
    return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
  }
  return `${(bytes / 1024 / 1024 / 1024).toFixed(1)} GB`;
}

const EXT_CATEGORY_COLORS: Array<[Set<string>, string]> = [
  [new Set(["pdf"]), "#e06060"],
  [new Set(["zip", "rar", "7z", "gz", "tar", "dmg", "iso"]), "#e0a050"],
  [
    new Set(["png", "jpg", "jpeg", "gif", "webp", "svg", "heic", "tiff", "psd"]),
    "#a078e0",
  ],
  [new Set(["mp4", "mov", "avi", "mkv", "mp3", "wav", "aiff", "flac"]), "#e078a8"],
  [
    new Set(["js", "jsx", "ts", "tsx", "rs", "py", "go", "rb", "java", "c", "h", "cpp", "swift", "kt"]),
    "#6bc8f0",
  ],
  [new Set(["json", "yml", "yaml", "toml", "xml", "html", "css", "sh", "sql"]), "#6be088"],
  [new Set(["md", "txt", "rtf", "doc", "docx", "pages"]), "#68b8f8"],
  [new Set(["xls", "xlsx", "csv", "numbers"]), "#78d890"],
  [new Set(["ppt", "pptx", "key"]), "#f0a058"],
];

function extColor(fileName: string) {
  const dot = fileName.lastIndexOf(".");
  if (dot <= 0 || dot === fileName.length - 1) {
    return "#9aa0a6";
  }
  const ext = fileName.slice(dot + 1).toLowerCase();
  for (const [exts, color] of EXT_CATEGORY_COLORS) {
    if (exts.has(ext)) {
      return color;
    }
  }
  return "#9aa0a6";
}

function StagedFileRow({
  file,
  copied,
  selected,
  onCopy,
  onReveal,
  onRemove,
  onToggleSelect,
  onDragOut,
}: {
  file: StagedFile;
  copied: boolean;
  onCopy: (id: string) => void;
  onReveal: (path: string) => void;
  onRemove: (id: string) => void;
  onToggleSelect: (id: string) => void;
  onDragOut: (rowId: string) => void;
  selected: boolean;
}) {
  const { t } = useTranslation();
  const lost = file.lost;

  // Drag-out gesture: once the pointer moves >8px with the button still held,
  // hand off to the native drag session; the click-to-copy that would follow
  // pointer-up is suppressed (a native drag usually swallows it anyway).
  const gestureRef = useRef<{ x: number; y: number; dragged: boolean } | null>(null);

  const handlePointerDown = (event: React.PointerEvent<HTMLButtonElement>) => {
    gestureRef.current =
      event.pointerType === "mouse" && event.button === 0 && !lost
        ? { x: event.clientX, y: event.clientY, dragged: false }
        : null;
  };

  const handlePointerMove = (event: React.PointerEvent<HTMLButtonElement>) => {
    const gesture = gestureRef.current;
    if (!gesture || gesture.dragged) {
      return;
    }
    const distance = Math.hypot(event.clientX - gesture.x, event.clientY - gesture.y);
    if (distance >= 8) {
      gesture.dragged = true;
      onDragOut(file.id);
    }
  };

  const handlePointerUp = () => {
    if (gestureRef.current && !gestureRef.current.dragged) {
      gestureRef.current = null;
    }
  };

  const meta = copied
    ? t("fileStation.copied")
    : lost
      ? t("fileStation.lost")
      : [
          file.isDir ? t("fileStation.folder") : formatBytes(file.byteSize),
          timeAgoFromMs(file.stagedAt),
        ]
          .filter(Boolean)
          .join(" · ");

  return (
    <div className={`fs-entry-row${lost ? " is-lost" : ""}${copied ? " is-copied" : ""}`}>
      <button
        type="button"
        className={`fs-entry${lost ? " is-lost" : ""}${copied ? " is-copied" : ""}${selected ? " is-selected" : ""}`}
        onPointerDown={handlePointerDown}
        onPointerMove={handlePointerMove}
        onPointerUp={handlePointerUp}
        onClick={(event) => {
          if (lost || gestureRef.current?.dragged) {
            return;
          }
          // Ctrl（Windows）与 Cmd（macOS）+点击：多选切换；普通点击仍是复制。
          if (event.ctrlKey || event.metaKey) {
            onToggleSelect(file.id);
            return;
          }
          onCopy(file.id);
        }}
        onContextMenu={(event) => event.preventDefault()}
        disabled={lost}
        data-no-drag
        title={file.path}
      >
        <span className="fs-entry-glyph" style={{ color: extColor(file.fileName) }}>
          <FolderOpen size={15} strokeWidth={2.1} />
        </span>
        <span className="fs-entry-body">
          <span className="fs-entry-name">{file.fileName}</span>
          <span className="fs-entry-meta">{meta}</span>
        </span>
      </button>
      <button
        type="button"
        className="fs-row-action"
        onClick={(event) => {
          event.preventDefault();
          event.stopPropagation();
          onReveal(file.path);
        }}
        disabled={lost}
        aria-label={t("fileStation.reveal")}
        title={t("fileStation.reveal")}
        data-no-drag
      >
        <Search size={13} />
      </button>
      <button
        type="button"
        className="fs-row-action fs-remove"
        onClick={(event) => {
          event.preventDefault();
          event.stopPropagation();
          onRemove(file.id);
        }}
        aria-label={t("fileStation.remove")}
        title={t("fileStation.remove")}
        data-no-drag
      >
        <X size={14} />
      </button>
    </div>
  );
}

export function FileStationView({
  files,
  onCopy,
  onReveal,
  onRemove,
  onClear,
  onDragOut,
}: FileStationViewProps) {
  const { t } = useTranslation();
  const [search, setSearch] = useState("");
  const [copiedId, setCopiedId] = useState<string | null>(null);
  const [confirmClear, setConfirmClear] = useState(false);
  // 多选：Ctrl/Cmd+点击切换；从选中行拖出时整批带走。
  const [selectedIds, setSelectedIds] = useState<ReadonlySet<string>>(new Set());

  // 文件被移除/清空后同步收缩选中集。
  useEffect(() => {
    setSelectedIds((prev) => {
      const live = new Set(files.map((f) => f.id));
      const next = new Set([...prev].filter((id) => live.has(id)));
      return next.size === prev.size ? prev : next;
    });
  }, [files]);

  // 拖出解析：从选中行发起 = 整批（按列表顺序）；未选中行 = 单个。
  const resolveDragIds = (rowId: string): string[] => {
    if (selectedIds.has(rowId)) {
      return files.filter((f) => selectedIds.has(f.id)).map((f) => f.id);
    }
    return [rowId];
  };

  const filtered = useMemo(() => {
    if (!search.trim()) return files;
    const q = search.toLowerCase();
    return files.filter(
      (file) =>
        file.fileName.toLowerCase().includes(q) ||
        file.path.toLowerCase().includes(q),
    );
  }, [files, search]);

  const handleCopy = (id: string) => {
    onCopy(id);
    setCopiedId(id);
    window.setTimeout(() => setCopiedId(null), 1200);
  };

  const handleClearClick = () => {
    if (!confirmClear) {
      setConfirmClear(true);
      window.setTimeout(() => setConfirmClear(false), 2500);
      return;
    }
    setConfirmClear(false);
    onClear();
  };

  return (
    <div className="file-station-view settings-view" data-no-drag>
      <div className="settings-body">
        {files.length === 0 ? (
          <div className="fs-empty">
            <div className="fs-empty-icon">
              <Inbox size={24} />
            </div>
            <p>{t("fileStation.empty")}</p>
            <p className="fs-empty-hint">{t("fileStation.emptyHint")}</p>
          </div>
        ) : (
          <>
            <div className="fs-toolbar">
              <span className="fs-count" title={t("fileStation.title")}>
                {files.length}/{STAGED_FILES_LIMIT}
              </span>
              <div className="clipboard-search-wrap">
                <Search size={13} className="clipboard-search-icon" />
                <input
                  type="text"
                  className="clipboard-search-input"
                  placeholder={t("fileStation.searchPlaceholder")}
                  value={search}
                  onChange={(e) => setSearch(e.target.value)}
                />
              </div>
              <button
                type="button"
                className={`clipboard-clear-btn${confirmClear ? " is-confirming" : ""}`}
                onClick={handleClearClick}
                data-no-drag
              >
                <Trash2 size={13} />
                <span>{confirmClear ? t("fileStation.clearConfirm") : t("fileStation.clear")}</span>
              </button>
            </div>

            {filtered.length === 0 ? (
              <div className="fs-empty">
                <div className="fs-empty-icon">
                  <Copy size={20} />
                </div>
                <p>{t("fileStation.noResults")}</p>
              </div>
            ) : (
              <div className="fs-list">
                {filtered.map((file) => (
                  <StagedFileRow
                    key={file.id}
                    file={file}
                    copied={copiedId === file.id}
                    selected={selectedIds.has(file.id)}
                    onCopy={handleCopy}
                    onReveal={onReveal}
                    onRemove={onRemove}
                    onToggleSelect={(id) => {
                      setSelectedIds((prev) => {
                        const next = new Set(prev);
                        if (next.has(id)) {
                          next.delete(id);
                        } else {
                          next.add(id);
                        }
                        return next;
                      });
                    }}
                    onDragOut={(rowId) => onDragOut(resolveDragIds(rowId))}
                  />
                ))}
              </div>
            )}
            <p className="fs-drag-hint">
              {t("fileStation.dragHint")}
              <br />
              {t("fileStation.multiSelectHint")}
            </p>
          </>
        )}
      </div>
    </div>
  );
}
