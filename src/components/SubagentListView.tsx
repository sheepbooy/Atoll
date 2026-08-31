import {
  memo,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  CSSProperties,
  UIEvent as ReactUIEvent,
} from "react";
import {
  Archive,
  Check,
  ChevronRight,
  Layers,
} from "lucide-react";
import {
  useTranslation,
} from "react-i18next";
import {
  type SubagentSummary,
} from "../tauri";
import {
  AgentMascot,
} from "../AgentMascot";
import {
  getSubagentColor,
  getSubagentMood,
} from "../subagentIdentity";
import {
  type AgentKind,
} from "../appTypes";
import {
  timeAgo,
} from "../sessionDisplay";

export interface SubagentListViewProps {
  subagents: SubagentSummary[];
  agent: AgentKind;
  onSelectSubagent: (agentId: string) => void;
  onArchiveCompletedSubagents: () => void | Promise<void>;
}

export const SUBAGENT_LIST_VIRTUALIZE_THRESHOLD = 40;
export const SUBAGENT_LIST_ROW_HEIGHT = 52;
export const SUBAGENT_LIST_OVERSCAN = 6;
export const SUBAGENT_LIST_FALLBACK_VIEWPORT_HEIGHT = 260;

export function sortSubagents(subagents: SubagentSummary[]) {
  return [...subagents].sort((a, b) => {
    const aDone = Boolean(a.completedAt);
    const bDone = Boolean(b.completedAt);
    if (aDone !== bDone) return aDone ? 1 : -1;
    return b.startedAt.localeCompare(a.startedAt);
  });
}

export interface SubagentListRowProps {
  subagent: SubagentSummary;
  agent: AgentKind;
  onSelectSubagent: (agentId: string) => void;
  style?: CSSProperties;
}

export const SubagentListRow = memo(function SubagentListRow({
  subagent,
  agent,
  onSelectSubagent,
  style,
}: SubagentListRowProps) {
  const { t } = useTranslation();
  const completed = Boolean(subagent.completedAt);
  const color = getSubagentColor(subagent.agentId);
  const mood = getSubagentMood(subagent.agentId, completed);
  const handleClick = useCallback(() => {
    onSelectSubagent(subagent.agentId);
  }, [onSelectSubagent, subagent.agentId]);

  return (
    <button
      type="button"
      className={`subagent-list-item ${color.tone} ${completed ? "is-completed" : ""}`}
      onClick={handleClick}
      style={style}
    >
      <AgentMascot
        agent={agent}
        size={18}
        mood={mood}
        accent={color.accent}
        accentDark={color.accentDark}
        animated={false}
      />
      <div className="subagent-list-item-info">
        <span className={`subagent-list-item-name ${color.tone}`}>
          {subagent.agentType}
        </span>
        <span className="subagent-list-item-meta">
          {timeAgo(subagent.startedAt)}
          {subagent.lastMessage ? (
            <>
              <span className="meta-divider">·</span>
              <span className="subagent-list-item-last-msg">
                {subagent.lastMessage}
              </span>
            </>
          ) : null}
        </span>
      </div>
      <div className="subagent-list-item-trail">
        {completed ? (
          <span className="subagent-status-badge done">
            <Check size={10} /> {t("subagent.done")}
          </span>
        ) : (
          <span className="subagent-status-badge running">{t("subagent.running")}</span>
        )}
        <ChevronRight size={14} />
      </div>
    </button>
  );
});

export function SubagentListView({
  subagents,
  agent,
  onSelectSubagent,
  onArchiveCompletedSubagents,
}: SubagentListViewProps) {
  const { t } = useTranslation();
  const listRef = useRef<HTMLDivElement>(null);
  const [scrollTop, setScrollTop] = useState(0);
  const [viewportHeight, setViewportHeight] = useState(0);
  const [archiveBusy, setArchiveBusy] = useState(false);
  const listState = useMemo(() => {
    let runningCount = 0;
    let hasCompleted = false;
    for (const subagent of subagents) {
      if (subagent.completedAt) {
        hasCompleted = true;
      } else {
        runningCount += 1;
      }
    }
    return {
      sorted: sortSubagents(subagents),
      runningCount,
      hasCompleted,
    };
  }, [subagents]);
  const { sorted, runningCount, hasCompleted } = listState;
  const shouldVirtualize = sorted.length > SUBAGENT_LIST_VIRTUALIZE_THRESHOLD;
  const effectiveViewportHeight =
    viewportHeight || SUBAGENT_LIST_FALLBACK_VIEWPORT_HEIGHT;
  const maxScrollTop = Math.max(
    0,
    sorted.length * SUBAGENT_LIST_ROW_HEIGHT - effectiveViewportHeight,
  );
  const virtualScrollTop = shouldVirtualize
    ? Math.min(scrollTop, maxScrollTop)
    : 0;
  const visibleStart = shouldVirtualize
    ? Math.max(
        0,
        Math.floor(virtualScrollTop / SUBAGENT_LIST_ROW_HEIGHT) -
          SUBAGENT_LIST_OVERSCAN,
      )
    : 0;
  const visibleEnd = shouldVirtualize
    ? Math.min(
        sorted.length,
        Math.ceil(
          (virtualScrollTop + effectiveViewportHeight) /
            SUBAGENT_LIST_ROW_HEIGHT,
        ) + SUBAGENT_LIST_OVERSCAN,
      )
    : sorted.length;
  const visibleSubagents = shouldVirtualize
    ? sorted.slice(visibleStart, visibleEnd)
    : sorted;

  useEffect(() => {
    const listEl = listRef.current;
    if (!listEl) return;

    const updateViewport = () => {
      setViewportHeight(listEl.clientHeight);
    };
    updateViewport();

    if (typeof ResizeObserver !== "undefined") {
      const observer = new ResizeObserver(updateViewport);
      observer.observe(listEl);
      return () => observer.disconnect();
    }

    window.addEventListener("resize", updateViewport);
    return () => window.removeEventListener("resize", updateViewport);
  }, []);

  useEffect(() => {
    if (!shouldVirtualize && scrollTop !== 0) {
      setScrollTop(0);
      return;
    }
    if (shouldVirtualize && scrollTop > maxScrollTop) {
      setScrollTop(maxScrollTop);
    }
  }, [maxScrollTop, scrollTop, shouldVirtualize]);

  const handleScroll = useCallback((event: ReactUIEvent<HTMLDivElement>) => {
    setScrollTop(event.currentTarget.scrollTop);
  }, []);

  async function handleArchiveCompletedSubagents() {
    if (archiveBusy) return;
    setArchiveBusy(true);
    try {
      await onArchiveCompletedSubagents();
    } finally {
      setArchiveBusy(false);
    }
  }

  return (
    <div className="subagent-list-view">
      <div className="subagent-list-header">
        <div className="subagent-list-title-row">
          <Layers size={14} />
          <span className="subagent-list-title">
            {t("subagent.listTitle", { count: subagents.length })}
          </span>
          {runningCount > 0 ? (
            <span className="subagent-list-running-badge">
              {t("subagent.runningBadge", { count: runningCount })}
            </span>
          ) : null}
        </div>
        {hasCompleted ? (
          <button
            type="button"
            className="subagent-list-archive-all-btn"
            onClick={handleArchiveCompletedSubagents}
            disabled={archiveBusy}
          >
            <Archive size={12} />
            <span>
              {archiveBusy ? t("subagent.archiving") : t("subagent.archiveCompleted")}
            </span>
          </button>
        ) : null}
      </div>
      <div
        ref={listRef}
        className={`subagent-list-body ${shouldVirtualize ? "is-virtualized" : ""}`}
        onScroll={handleScroll}
      >
        {shouldVirtualize ? (
          <div
            className="subagent-list-virtual-spacer"
            style={{ height: sorted.length * SUBAGENT_LIST_ROW_HEIGHT }}
          >
            <div
              className="subagent-list-virtual-window"
              style={{
                transform: `translateY(${
                  visibleStart * SUBAGENT_LIST_ROW_HEIGHT
                }px)`,
              }}
            >
              {visibleSubagents.map((subagent) => (
                <SubagentListRow
                  key={subagent.agentId}
                  subagent={subagent}
                  agent={agent}
                  onSelectSubagent={onSelectSubagent}
                  style={{ height: SUBAGENT_LIST_ROW_HEIGHT }}
                />
              ))}
            </div>
          </div>
        ) : (
          visibleSubagents.map((subagent) => (
            <SubagentListRow
              key={subagent.agentId}
              subagent={subagent}
              agent={agent}
              onSelectSubagent={onSelectSubagent}
            />
          ))
        )}
      </div>
    </div>
  );
}

