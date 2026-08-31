import {
  useEffect,
  useRef,
  useState,
} from "react";
import {
  Archive,
  Check,
} from "lucide-react";
import {
  useTranslation,
} from "react-i18next";
import {
  getSessionTranscript,
  type ChatMessage,
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
import {
  ChatBubble,
} from "../components/ChatBubble";

export interface SubagentDetailViewProps {
  agentId: string;
  agent: AgentKind;
  agentType: string;
  startedAt: string;
  completedAt: string | null;
  lastMessage: string | null;
  transcriptPath: string | null;
  onArchive: () => void | Promise<void>;
}

export function SubagentDetailView({
  agentId,
  agent,
  agentType,
  startedAt,
  completedAt,
  lastMessage,
  transcriptPath,
  onArchive,
}: SubagentDetailViewProps) {
  const { t } = useTranslation();
  const subagentColor = getSubagentColor(agentId);
  const subagentMood = getSubagentMood(agentId, Boolean(completedAt));
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [loadFailed, setLoadFailed] = useState(false);
  const [archiveBusy, setArchiveBusy] = useState(false);
  const scrollRef = useRef<HTMLDivElement>(null);
  const prevCountRef = useRef(0);

  useEffect(() => {
    if (!transcriptPath) return;
    let active = true;
    setLoadFailed(false);
    function load() {
      getSessionTranscript(transcriptPath!)
        .then((msgs) => {
          if (!active) return;
          setLoadFailed(false);
          if (msgs.length !== prevCountRef.current) {
            prevCountRef.current = msgs.length;
            setMessages(msgs);
          }
        })
        .catch(() => {
          if (!active) return;
          setLoadFailed(true);
        });
    }
    load();
    const pollMs = completedAt ? 0 : 2000;
    const interval = pollMs > 0 ? setInterval(load, pollMs) : undefined;
    return () => {
      active = false;
      if (interval) clearInterval(interval);
    };
  }, [transcriptPath, completedAt]);

  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [messages]);

  async function handleArchive() {
    if (archiveBusy) return;
    setArchiveBusy(true);
    try {
      await onArchive();
    } finally {
      setArchiveBusy(false);
    }
  }

  return (
    <div className="subagent-detail-view">
      <div className="subagent-detail-header">
        <div className="subagent-detail-title-row">
          <AgentMascot
            agent={agent}
            size={20}
            mood={subagentMood}
            accent={subagentColor.accent}
            accentDark={subagentColor.accentDark}
          />
          <h2 className={`subagent-detail-title ${subagentColor.tone}`}>{agentType}</h2>
          {completedAt ? (
            <span className="subagent-status-badge done">
              <Check size={10} /> {t("subagent.done")}
            </span>
          ) : (
            <span className="subagent-status-badge running">{t("subagent.running")}</span>
          )}
        </div>
        <p className="subagent-detail-subtitle">
          {t("subagent.started", { timeAgo: timeAgo(startedAt) })}
          {completedAt
            ? ` ${t("subagent.finished", { timeAgo: timeAgo(completedAt) })}`
            : ""}
        </p>
        {lastMessage && messages.length === 0 ? (
          <p className="subagent-last-message">{lastMessage}</p>
        ) : null}
      </div>
      <div className="session-chat">
        <div className="chat-messages" ref={scrollRef}>
          {messages.length === 0 && !lastMessage ? (
            <div className="chat-empty">
              {!transcriptPath
                ? t("subagent.noTranscriptPath")
                : loadFailed && completedAt
                  ? t("chat.transcriptUnavailable")
                  : t("subagent.loading")}
            </div>
          ) : null}
          {messages.map((msg, i) => (
            <ChatBubble key={i} message={msg} />
          ))}
        </div>
      </div>
      {completedAt ? (
        <div className="subagent-detail-footer">
          <button
            type="button"
            className="subagent-archive-btn"
            onClick={handleArchive}
            disabled={archiveBusy}
          >
            <Archive size={14} />
            <span>{archiveBusy ? t("subagent.archiving") : t("subagent.archive")}</span>
          </button>
        </div>
      ) : null}
    </div>
  );
}

