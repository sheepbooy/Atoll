import i18n from "./i18n";

export function sessionDisplayName(cwd: string) {
  if (!cwd || cwd === ".") {
    return i18n.t("session.cursorSession");
  }
  const parts = cwd.split(/[/\\]/).filter(Boolean);
  return parts[parts.length - 1] || cwd;
}

export function timeAgo(isoDate: string) {
  const elapsedSeconds = Math.max(1, Math.floor((Date.now() - new Date(isoDate).getTime()) / 1000));

  if (elapsedSeconds < 60) {
    return i18n.t("time.agoSeconds", { seconds: elapsedSeconds });
  }

  const elapsedMinutes = Math.floor(elapsedSeconds / 60);
  if (elapsedMinutes < 60) {
    return i18n.t("time.agoMinutes", { minutes: elapsedMinutes });
  }

  const elapsedHours = Math.floor(elapsedMinutes / 60);
  return i18n.t("time.agoHours", { hours: elapsedHours });
}
