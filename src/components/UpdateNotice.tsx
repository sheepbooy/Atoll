import {
  CircleCheck,
} from "lucide-react";
import {
  useTranslation,
} from "react-i18next";

export function UpdateNotice({
  version,
  onDismiss,
}: {
  version: string;
  onDismiss: () => void;
}) {
  const { t } = useTranslation();

  return (
    <div
      className="update-notice-layer"
      data-no-drag
      onMouseDown={(event) => event.stopPropagation()}
      onClick={onDismiss}
    >
      <div
        className="update-notice-card"
        role="alertdialog"
        aria-labelledby="update-notice-title"
        aria-describedby="update-notice-desc"
        onClick={(event) => event.stopPropagation()}
      >
        <div className="update-notice-icon-wrap" aria-hidden="true">
          <CircleCheck size={28} strokeWidth={1.75} />
        </div>
        <p id="update-notice-title" className="update-notice-title">
          {t("update.noticeTitle")}
        </p>
        <p id="update-notice-desc" className="update-notice-desc">
          {t("update.noticeDesc", { version })}
        </p>
        <button type="button" className="update-notice-button" onClick={onDismiss}>
          {t("update.ok")}
        </button>
      </div>
    </div>
  );
}
