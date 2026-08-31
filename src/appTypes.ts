import {
  type PermissionRequest,
} from "./tauri";

// Shared island UI types used by App and its extracted components.
export type Decision = "approved" | "denied";
export type AgentKind = PermissionRequest["agent"];
export type PanelView =
  | { kind: "home" }
  | { kind: "session"; sessionId: string }
  | { kind: "subagent"; sessionId: string; agentId: string }
  | { kind: "subagentList"; sessionId: string }
  | { kind: "settings"; page: SettingsPage }
  | { kind: "clipboard" }
  | { kind: "history" };
export type SettingsPage =
  | "main"
  | "hooks"
  | "tokens"
  | "usage"
  | "island"
  | "media"
  | "clipboard"
  | "sessions"
  | "mascot"
  | "notifications"
  | "shortcuts";
export type FoldedIslandSize = "small" | "regular";
// Window-space rect of the compact media thumb plus the window size it was
// measured against; scales the expanded artwork backdrop back onto the thumb.
export type ArtworkBackdropOrigin = {
  x: number;
  y: number;
  w: number;
  h: number;
  winW: number;
  winH: number;
};
