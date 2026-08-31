import {
  useEffect,
  useRef,
  useState,
} from "react";
import {
  useTranslation,
} from "react-i18next";
import {
  getSessionChat,
  getSessionTranscript,
  type PermissionRequest,
  type ChatMessage,
} from "../tauri";
import {
  type AgentKind,
} from "../appTypes";
import {
  ChatBubble,
} from "../components/ChatBubble";

export interface SessionChatViewProps {
  sessionId: string;
  transcriptPath: string | null;
  requests: PermissionRequest[];
  agent: AgentKind;
}

export function SessionChatView({ sessionId, transcriptPath, requests, agent }: SessionChatViewProps) {
  const { t } = useTranslation();
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [loadFailed, setLoadFailed] = useState(false);
  const scrollRef = useRef<HTMLDivElement>(null);
  const pollWhileActive = agent === "cursor" || agent === "zcode";

  useEffect(() => {
    let active = true;
    let loading = false;
    setLoadFailed(false);

    function loadByPath(path: string) {
      return getSessionTranscript(path)
        .then((msgs) => {
          if (!active) return;
          setLoadFailed(false);
          setMessages(msgs);
        })
        .catch(() => {
          if (!active) return;
          setLoadFailed(true);
        });
    }

    function loadBySession() {
      return getSessionChat(sessionId)
        .then((msgs) => {
          if (!active) return;
          setLoadFailed(false);
          setMessages(msgs);
        })
        .catch(() => {
          if (!active) return;
          setLoadFailed(true);
        });
    }

    function load() {
      if (loading) {
        return Promise.resolve();
      }
      loading = true;
      const request = transcriptPath
        ? loadByPath(transcriptPath)
        : pollWhileActive
          ? loadBySession()
          : Promise.resolve();
      return request.finally(() => {
        loading = false;
      });
    }

    function loadAndIgnore() {
      void load();
    }

    loadAndIgnore();
    const interval = pollWhileActive ? window.setInterval(loadAndIgnore, 2000) : undefined;
    return () => {
      active = false;
      if (interval !== undefined) {
        window.clearInterval(interval);
      }
    };
  }, [sessionId, transcriptPath, pollWhileActive]);

  useEffect(() => {
    if (!scrollRef.current) {
      return;
    }
    scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
  }, [messages]);

  return (
    <div className="session-chat">
      <div className="chat-messages" ref={scrollRef}>
        {messages.length === 0 && requests.length === 0 ? (
          <div className="chat-empty">
            {loadFailed
              ? t("chat.transcriptUnavailable")
              : pollWhileActive || transcriptPath
                ? t("chat.loading")
                : t("chat.noHistory")}
          </div>
        ) : null}
        {messages.map((msg, i) => (
          <ChatBubble key={i} message={msg} />
        ))}
      </div>
    </div>
  );
}

