import type { AtollActivity } from "./AtollLogo";

/**
 * App-visible logo states (menu bar / island indicator).
 * Atoll Logo 只反映这四种全局状态；与 session 级 Clawd 等形象无关。
 */
export type AppLogoState = "idle" | "pending" | "working" | "offline";

export const APP_LOGO_STATE_LABELS: Record<AppLogoState, string> = {
  idle: "空闲",
  pending: "待审批",
  working: "工作中",
  offline: "离线",
};

/** 触发条件说明（与 deriveAppLogoState 优先级一致）。 */
export const APP_STATE_TRIGGERS: Record<AppLogoState, string> = {
  offline: "Hook 未安装/已卸载，或 hook 脚本缺失，或本地 bridge 不可达",
  pending: "有待审批项",
  working: "有活跃 Agent 会话",
  idle: "在线监听中（Hook 就绪且 bridge 正常），且无待审批、无活跃会话",
};

export const APP_LOGO_STATES: AppLogoState[] = [
  "idle",
  "pending",
  "working",
  "offline",
];

/** Logo 动画 pose（应用态 + 彩蛋共用同一套 AtollActivity 类型）。 */
export const ACTIVITY_LABELS: Record<AtollActivity, string> = {
  idle: "idle",
  coding: "coding",
  reading: "reading",
  thinking: "thinking",
  coffee: "coffee",
  idea: "idea",
  slacking: "slacking",
  napping: "napping",
};

export const ACTIVITY_HINTS: Record<AtollActivity, string> = {
  idle: "无腿 · 轻浮",
  coding: "小桌键盘 · 隐形手打字",
  reading: "胸前捧书 · 隐形手翻页",
  thinking: "问号气泡 · 隐形手托腮",
  coffee: "举杯靠近嘴边 · 隐形手",
  idea: "头顶灯泡 · 隐形手举顶",
  slacking: "墨镜 + 举手机 · 隐形手",
  napping: "睡帽 zzz · 无腿",
};

/** 四种应用态 → Logo pose（由 App 条件直接驱动，非随机）。 */
export const APP_STATE_ACTIVITY_MAP: Record<AppLogoState, AtollActivity> = {
  idle: "idle",
  pending: "thinking",
  working: "coding",
  offline: "napping",
};

/**
 * 空闲彩蛋：仅在应用态为「空闲」时，按设置中的间隔/时长随机播放。
 * 与应用态 pose 互不重叠；具体包含哪些活动后续可调整。
 */
export const IDLE_EASTER_EGG_ACTIVITIES: AtollActivity[] = [
  "reading",
  "coffee",
  "idea",
  "slacking",
];

export function appStateToActivity(state: AppLogoState): AtollActivity {
  return APP_STATE_ACTIVITY_MAP[state];
}

export function activityToAppState(activity: AtollActivity): AppLogoState | null {
  switch (activity) {
    case "idle":
      return "idle";
    case "coding":
      return "working";
    case "thinking":
      return "pending";
    case "napping":
      return "offline";
    default:
      return null;
  }
}

export function isEasterEggActivity(activity: AtollActivity): boolean {
  return IDLE_EASTER_EGG_ACTIVITIES.includes(activity);
}

export function isAppStatePose(activity: AtollActivity): boolean {
  return activityToAppState(activity) !== null;
}

/** 当前 App 条件是否处于可触发空闲彩蛋的状态。 */
export function canTriggerIdleEasterEgg(input: {
  online: boolean;
  pendingCount: number;
  sessionCount: number;
}): boolean {
  return deriveAppLogoState(input) === "idle";
}

export function deriveAppLogoState(input: {
  online: boolean;
  pendingCount: number;
  sessionCount: number;
}): AppLogoState {
  if (!input.online) return "offline";
  if (input.pendingCount > 0) return "pending";
  if (input.sessionCount > 0) return "working";
  return "idle";
}

export function deriveAtollActivity(input: {
  online: boolean;
  pendingCount: number;
  sessionCount: number;
}): AtollActivity {
  return appStateToActivity(deriveAppLogoState(input));
}
