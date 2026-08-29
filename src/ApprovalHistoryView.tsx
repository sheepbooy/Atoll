import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  ChevronDown,
  ChevronUp,
  FileJson,
  FileSpreadsheet,
  FolderOpen,
  History,
  Search,
  Trash2,
  X,
} from "lucide-react";
import i18n from "./i18n";
import {
  clearApprovalHistory,
  exportApprovalHistory,
  getApprovalHistory,
  revealPath,
  type ApprovalHistoryEntry,
  type ApprovalHistoryQuery,
  type ApprovalHistoryStatus,
} from "./tauri";

const PAGE_SIZE = 50;
const SEARCH_DEBOUNCE_MS = 300;
const TOAST_AUTO_DISMISS_MS = 6000;

const AGENT_FILTERS = ["", "claude", "codex", "cursor", "zcode"] as const;
const OUTCOME_FILTERS = [
  "",
  "approved",
  "denied",
  "expired",
  "answered_elsewhere",
] as const;

const OUTCOME_LABEL_KEYS: Record<ApprovalHistoryStatus, string> = {
  pending: "history.outcomePending",
  approved: "history.outcomeApproved",
  denied: "history.outcomeDenied",
  expired: "history.outcomeExpired",
  answered_elsewhere: "history.outcomeAnsweredElsewhere",
};

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
  if (hours < 48) {
    return i18n.t("time.agoHours", { hours });
  }
  const days = Math.floor(hours / 24);
  if (days < 7) {
    return i18n.t("time.agoDays", { days });
  }
  return new Intl.DateTimeFormat(i18n.language, {
    month: "short",
    day: "numeric",
  }).format(new Date(unixSecs * 1000));
}

function projectName(cwd: string) {
  const trimmed = cwd.replace(/[\\/]+$/, "");
  const base = trimmed.split(/[\\/]/).pop();
  return base && base !== "." ? base : trimmed || cwd;
}

function shortSessionId(sessionId: string) {
  return sessionId.length > 12 ? `${sessionId.slice(0, 12)}…` : sessionId;
}

function agentLabelKey(agent: string) {
  switch (agent) {
    case "claude":
      return "history.agentClaude";
    case "codex":
      return "history.agentCodex";
    case "cursor":
      return "history.agentCursor";
    case "zcode":
      return "history.agentZcode";
    case "gemini":
      return "history.agentGemini";
    default:
      return "history.agentOther";
  }
}

interface ToastState {
  text: string;
  path?: string;
}

function HistoryEntryRow({
  entry,
  expanded,
  onToggleExpand,
  onSelectSession,
}: {
  entry: ApprovalHistoryEntry;
  expanded: boolean;
  onToggleExpand: (id: string) => void;
  onSelectSession: (sessionId: string) => void;
}) {
  const { t } = useTranslation();
  const requestedLabel = timeAgoFromSecs(entry.requestedAt);
  return (
    <div className={`history-entry is-${entry.status}${expanded ? " is-expanded" : ""}`}>
      <button
        type="button"
        className="history-entry-main"
        onClick={() => onToggleExpand(entry.id)}
        aria-expanded={expanded}
        data-no-drag
      >
        <span className={`history-badge is-${entry.status}`}>
          {t(OUTCOME_LABEL_KEYS[entry.status])}
        </span>
        <span className="history-entry-body">
          <span className="history-entry-command">{entry.command}</span>
          <span className="history-entry-meta">
            {t(agentLabelKey(entry.agent))} · {projectName(entry.cwd)} ·{" "}
            {requestedLabel}
          </span>
        </span>
        {expanded ? (
          <ChevronUp size={14} className="history-chevron" />
        ) : (
          <ChevronDown size={14} className="history-chevron" />
        )}
      </button>
      {expanded ? (
        <div className="history-entry-details">
          <div className="history-detail-row">
            <span className="history-detail-label">{t("history.detailCommand")}</span>
            <span className="history-detail-value">{entry.command}</span>
          </div>
          <div className="history-detail-row">
            <span className="history-detail-label">{t("history.detailProject")}</span>
            <span className="history-detail-value">{entry.cwd}</span>
          </div>
          <div className="history-detail-row">
            <span className="history-detail-label">{t("history.detailOutcome")}</span>
            <span className="history-detail-value">{entry.detail}</span>
          </div>
          <div className="history-detail-row">
            <span className="history-detail-label">{t("history.detailSession")}</span>
            <button
              type="button"
              className="history-session-chip"
              onClick={(event) => {
                event.stopPropagation();
                onSelectSession(entry.sessionId);
              }}
              data-no-drag
              title={t("history.sessionFilterTitle")}
            >
              {shortSessionId(entry.sessionId)}
            </button>
          </div>
          {entry.host ? (
            <div className="history-detail-row">
              <span className="history-detail-label">{t("history.detailHost")}</span>
              <span className="history-detail-value">{entry.host}</span>
            </div>
          ) : null}
        </div>
      ) : null}
    </div>
  );
}

export function ApprovalHistoryView() {
  const { t } = useTranslation();
  const [searchInput, setSearchInput] = useState("");
  const [search, setSearch] = useState("");
  const [agent, setAgent] = useState("");
  const [status, setStatus] = useState("");
  const [sessionId, setSessionId] = useState("");
  const [items, setItems] = useState<ApprovalHistoryEntry[]>([]);
  const [total, setTotal] = useState(0);
  const [hasLoaded, setHasLoaded] = useState(false);
  const [expandedId, setExpandedId] = useState<string | null>(null);
  const [toast, setToast] = useState<ToastState | null>(null);
  const requestSeq = useRef(0);
  const toastTimer = useRef<number | null>(null);

  // Debounce the search box: every keystroke re-queries SQLite otherwise.
  useEffect(() => {
    const timer = window.setTimeout(() => setSearch(searchInput.trim()), SEARCH_DEBOUNCE_MS);
    return () => window.clearTimeout(timer);
  }, [searchInput]);

  const baseQuery: ApprovalHistoryQuery = useMemo(
    () => ({
      search: search || undefined,
      agent: agent || undefined,
      status: status || undefined,
      sessionId: sessionId || undefined,
    }),
    [search, agent, status, sessionId],
  );

  const loadPage = useCallback(
    async (query: ApprovalHistoryQuery, append: boolean) => {
      const seq = ++requestSeq.current;
      try {
        const page = await getApprovalHistory(query);
        if (seq !== requestSeq.current) {
          return;
        }
        setItems((prev) => (append ? [...prev, ...page.items] : page.items));
        setTotal(page.total);
      } catch {
        if (seq === requestSeq.current && !append) {
          setItems([]);
          setTotal(0);
        }
      }
      if (seq === requestSeq.current) {
        setHasLoaded(true);
      }
    },
    [],
  );

  useEffect(() => {
    void loadPage({ ...baseQuery, limit: PAGE_SIZE, offset: 0 }, false);
  }, [baseQuery, loadPage]);

  const showToast = useCallback((next: ToastState) => {
    setToast(next);
    if (toastTimer.current !== null) {
      window.clearTimeout(toastTimer.current);
    }
    toastTimer.current = window.setTimeout(
      () => setToast(null),
      TOAST_AUTO_DISMISS_MS,
    );
  }, []);

  useEffect(() => {
    return () => {
      if (toastTimer.current !== null) {
        window.clearTimeout(toastTimer.current);
      }
    };
  }, []);

  const hasFilters = Boolean(search || agent || status || sessionId);
  const showEmpty = hasLoaded && items.length === 0;

  const handleExport = async (format: "json" | "csv") => {
    try {
      const path = await exportApprovalHistory(baseQuery, format);
      if (path) {
        showToast({ text: t("history.exported", { path }), path });
      } else {
        showToast({ text: t("history.exportUnavailable") });
      }
    } catch {
      showToast({ text: t("history.exportFailed") });
    }
  };

  const handleClear = async () => {
    try {
      await clearApprovalHistory();
      showToast({ text: t("history.cleared") });
      setSessionId("");
      setSearchInput("");
      setSearch("");
      setAgent("");
      setStatus("");
      await loadPage({ limit: PAGE_SIZE, offset: 0 }, false);
    } catch {
      showToast({ text: t("history.exportFailed") });
    }
  };

  const handleReveal = (path: string) => {
    revealPath(path).catch(() => undefined);
  };

  const handleToggleExpand = (id: string) => {
    setExpandedId((current) => (current === id ? null : id));
  };

  const handleSelectSession = (next: string) => {
    setSessionId((current) => (current === next ? "" : next));
  };

  const loadMore = () => {
    void loadPage(
      { ...baseQuery, limit: PAGE_SIZE, offset: items.length },
      true,
    );
  };

  return (
    <div className="approval-history-view settings-view" data-no-drag>
      <div className="settings-body">
        <div className="clipboard-toolbar history-toolbar">
          <div className="clipboard-search-wrap">
            <Search size={13} className="clipboard-search-icon" />
            <input
              type="text"
              className="clipboard-search-input"
              placeholder={t("history.searchPlaceholder")}
              value={searchInput}
              onChange={(e) => setSearchInput(e.target.value)}
              aria-label={t("history.searchPlaceholder")}
            />
          </div>
        </div>

        <div className="history-filters">
          <div className="history-filter-group" role="group" aria-label={t("history.agentFilter")}>
            {AGENT_FILTERS.map((value) => (
              <button
                key={value || "all"}
                type="button"
                className={`history-filter-pill${agent === value ? " is-active" : ""}`}
                onClick={() => setAgent(value)}
                data-no-drag
              >
                {value
                  ? t(agentLabelKey(value))
                  : t("history.filterAll")}
              </button>
            ))}
          </div>
          <div className="history-filter-group" role="group" aria-label={t("history.outcomeFilter")}>
            {OUTCOME_FILTERS.map((value) => (
              <button
                key={value || "all"}
                type="button"
                className={`history-filter-pill${status === value ? " is-active" : ""}`}
                onClick={() => setStatus(value)}
                data-no-drag
              >
                {value
                  ? t(OUTCOME_LABEL_KEYS[value as ApprovalHistoryStatus])
                  : t("history.filterAll")}
              </button>
            ))}
          </div>
          {sessionId ? (
            <div className="history-session-filter">
              <span className="history-session-filter-label">
                {t("history.sessionFilterLabel", { session: shortSessionId(sessionId) })}
              </span>
              <button
                type="button"
                className="history-session-clear"
                onClick={() => setSessionId("")}
                aria-label={t("history.sessionFilterClear")}
                data-no-drag
              >
                <X size={12} />
              </button>
            </div>
          ) : null}
        </div>

        {showEmpty ? (
          <div className="clipboard-empty">
            <div className="clipboard-empty-icon">
              <History size={24} />
            </div>
            <p>{hasFilters ? t("history.emptySearch") : t("history.empty")}</p>
          </div>
        ) : (
          <div className="history-list">
            {items.map((entry) => (
              <HistoryEntryRow
                key={entry.id}
                entry={entry}
                expanded={expandedId === entry.id}
                onToggleExpand={handleToggleExpand}
                onSelectSession={handleSelectSession}
              />
            ))}
            {items.length < total ? (
              <button type="button" className="history-load-more" onClick={loadMore} data-no-drag>
                {t("history.loadMore")}
              </button>
            ) : null}
          </div>
        )}

        <div className="history-footer">
          <button
            type="button"
            className="history-export-btn"
            onClick={() => void handleExport("json")}
            data-no-drag
          >
            <FileJson size={13} />
            <span>{t("history.exportJson")}</span>
          </button>
          <button
            type="button"
            className="history-export-btn"
            onClick={() => void handleExport("csv")}
            data-no-drag
          >
            <FileSpreadsheet size={13} />
            <span>{t("history.exportCsv")}</span>
          </button>
          <button
            type="button"
            className="history-clear-btn"
            onClick={() => void handleClear()}
            data-no-drag
          >
            <Trash2 size={13} />
            <span>{t("history.clear")}</span>
          </button>
        </div>

        {toast ? (
          <div className="history-toast" role="status">
            <span className="history-toast-text">{toast.text}</span>
            {toast.path ? (
              <button
                type="button"
                className="history-toast-reveal"
                onClick={() => handleReveal(toast.path as string)}
                data-no-drag
              >
                <FolderOpen size={12} />
                <span>{t("history.revealInFolder")}</span>
              </button>
            ) : null}
            <button
              type="button"
              className="history-toast-close"
              onClick={() => setToast(null)}
              aria-label={t("history.dismiss")}
              data-no-drag
            >
              <X size={12} />
            </button>
          </div>
        ) : null}
      </div>
    </div>
  );
}
