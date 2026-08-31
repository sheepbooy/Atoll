import {
  useMemo,
  MouseEvent,
} from "react";
import {
  useTranslation,
} from "react-i18next";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import {
  openUrl,
  type ChatMessage,
} from "../tauri";
import i18n from "../i18n";

export function ChatBubbleQuestionReadonly({
  toolInput,
  toolOutput,
}: {
  toolInput: unknown;
  toolOutput?: string | null;
}) {
  const { t } = useTranslation();
  const input = toolInput as { questions?: Array<{ question: string; header?: string; options?: Array<{ label: string; description?: string }>; multiSelect?: boolean }> } | null;
  const questions = input?.questions;
  if (!questions?.length) return null;
  const answer = toolOutput?.trim();

  return (
    <div className="chat-question-readonly">
      {questions.map((q, qi) => (
        <div key={qi} className="chat-question-item">
          {q.header && <span className="chat-question-header">{q.header}</span>}
          <span className="chat-question-text">{q.question}</span>
          {q.options && (
            <div className="chat-question-options">
              {q.options.map((opt, oi) => (
                <span key={oi} className="chat-question-option">{opt.label}</span>
              ))}
            </div>
          )}
        </div>
      ))}
      {answer && (
        <div className="chat-question-answer">
          <span className="chat-question-answer-label">{t("chat.yourAnswer")}</span>
          <span className="chat-question-answer-text">{answer}</span>
        </div>
      )}
    </div>
  );
}

export function ChatBubble({ message }: { message: ChatMessage }) {
  const text =
    message.content ||
    (message.toolName
      ? i18n.t("chat.usingTool", { toolName: message.toolName })
      : "");
  const hasMarkdown = useMemo(() => /[*_`#\[\]!\n>|]/.test(text), [text]);
  const isQuestion = message.toolName === "AskUserQuestion" && message.toolInput;

  function handleClick(event: MouseEvent<HTMLDivElement>) {
    const anchor = (event.target as HTMLElement).closest("a");
    if (anchor?.href) {
      event.preventDefault();
      event.stopPropagation();
      openUrl(anchor.href);
    }
  }

  return (
    <div className={`chat-bubble ${message.role}`} onClick={handleClick}>
      {message.toolName ? (
        <span className="chat-tool-badge">{message.toolName}</span>
      ) : null}
      {isQuestion ? (
        <ChatBubbleQuestionReadonly toolInput={message.toolInput} toolOutput={message.toolOutput} />
      ) : hasMarkdown ? (
        <div className="chat-bubble-md">
          <ReactMarkdown remarkPlugins={[remarkGfm]}>{text}</ReactMarkdown>
        </div>
      ) : (
        <span className="chat-bubble-text">{text}</span>
      )}
    </div>
  );
}
