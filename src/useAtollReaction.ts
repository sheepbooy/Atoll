import { useEffect, useRef, useState } from "react";
import type { AtollReaction } from "./AtollLogo";
import type { AppLogoState } from "./logoStates";

/**
 * 应用状态跃迁 → 一次性反应：
 * - 任意 → offline：collapse（挣扎后灰化瘫倒）
 * - offline → 其他：revive（苏醒回色甩头）
 * - pending/working → idle：cheer（✓ 星火起跳）
 */
export function deriveAtollReaction(
  prev: AppLogoState,
  next: AppLogoState,
  decision?: "approved" | "denied",
): AtollReaction | null {
  if (prev === next) return null;
  if (next === "offline") return "collapse";
  if (prev === "offline") return "revive";
  if (next === "idle") return decision === "denied" ? null : "cheer";
  return null;
}

/**
 * 跟踪 app 状态变化并派生一次性反应；重复触发同一反应时 reactionKey 递增以重放。
 */
export function useAtollReaction(state: AppLogoState, decision?: "approved" | "denied"): {
  reaction: AtollReaction | null;
  reactionKey: number;
} {
  const [reaction, setReaction] = useState<AtollReaction | null>(null);
  const [key, setKey] = useState(0);
  const prevRef = useRef<AppLogoState | null>(null);

  useEffect(() => {
    const prev = prevRef.current;
    prevRef.current = state;
    if (prev === null) return;
    const next = deriveAtollReaction(prev, state, decision);
    if (next) {
      setReaction(next);
      setKey((k) => k + 1);
    }
  }, [state, decision]);

  return { reaction, reactionKey: key };
}
