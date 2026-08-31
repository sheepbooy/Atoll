import {
  type PermissionRequest,
  type SessionSummary,
} from "./tauri";
import i18n from "./i18n";
import {
  type ClawdMood,
} from "./ClawdMascot";

export type RiskLevel = "danger" | "caution";

export const DANGER_PATTERNS: RegExp[] = [
  /\brm\s+(-\w*\s+)*-?\w*[rf]\w*[rf]/i,
  /\bsudo\b/i,
  /git\s+push\b[^\n]*(--force\b|\s-f\b|--force-with-lease\b)/i,
  /git\s+reset\s+--hard\b/i,
  /\bdd\s+if=/i,
  /\bmkfs\b/i,
  /:\(\)\s*\{[^}]*\}\s*;\s*:/,
  /chmod\s+-?\w*\s*777\b/i,
  /(curl|wget)[^|]*\|\s*(sudo\s+)?(ba|z|fi)?sh\b/i,
  />\s*\/dev\/(sd|disk|null|zero)/i,
  /\bkill(all)?\b|\bkill\s+-9\b/i,
  /\b(shutdown|reboot|halt|poweroff)\b/i,
  /\bDROP\s+(TABLE|DATABASE)\b/i,
  /\bTRUNCATE\s+TABLE\b/i,
  /Remove-Item\b[^\n]*(-Recurse|-Force)/i,
  /\bdel\s+\/f\b/i,
  /\bformat\s+[a-z]:/i,
];

export const CAUTION_PATTERNS: RegExp[] = [
  /\brm\s+-/i,
  /\bgit\s+clean\b/i,
  /\bgit\s+checkout\s+--\s/i,
  /\b(npm|pnpm|yarn|bun)\s+(install|i|ci|add|remove)\b/i,
  /\b(mv|chmod|chown|ln)\b/i,
  /\bdocker\b[^\n]*\b(rm|rmi|prune|down|stop)\b/i,
  /\b(brew|apt|apt-get|yum|dnf|pacman)\s+(install|remove|uninstall)\b/i,
  /\bpowershell\b[^\n]*(-ExecutionPolicy|-EncodedCommand)/i,
  />>?\s*[^\s|&]/,
];

export function assessRisk(command: string): RiskLevel | null {
  if (DANGER_PATTERNS.some((pattern) => pattern.test(command))) return "danger";
  if (CAUTION_PATTERNS.some((pattern) => pattern.test(command))) return "caution";
  return null;
}

export function localizedRiskLabel(risk: RiskLevel): string {
  return i18n.t(risk === "danger" ? "approval.riskHigh" : "approval.riskReview");
}

export function deriveSessionMood(
  session: SessionSummary,
  activeRequest: PermissionRequest | null,
  justResolved: boolean,
): ClawdMood {
  if (activeRequest && activeRequest.session === session.sessionId) {
    return assessRisk(activeRequest.command) === "danger" ? "worried" : "alert";
  }
  if (session.pendingCount > 0) return "alert";
  if (justResolved) return "happy";
  return "calm";
}
