import {
  type HeaderLogoDisplay,
} from "../hookHealth";
import {
  AgentMascot,
} from "../AgentMascot";
import {
  AtollLogo,
  type AtollReaction,
} from "../AtollLogo";
import {
  agentMascotAccent,
  agentMascotDark,
} from "../agents";

export interface HeaderLogoProps {
  display: HeaderLogoDisplay;
  size: number;
  idleIntervalSec: number;
  idleDurationSec: number;
  motionPaused: boolean;
  /** 一次性状态反应（仅 atoll 形象使用）；重复触发时递增 reactionKey 重放。 */
  reaction?: AtollReaction | null;
  reactionKey?: number;
}

export function HeaderLogo({
  display,
  size,
  idleIntervalSec,
  idleDurationSec,
  motionPaused,
  reaction = null,
  reactionKey = 0,
}: HeaderLogoProps) {
  if (display.kind === "agent") {
    return (
      <AgentMascot
        agent={display.agent}
        mood={display.mood}
        size={size}
        className="header-agent-logo"
        accent={agentMascotAccent(display.agent)}
        accentDark={agentMascotDark(display.agent)}
      />
    );
  }

  return (
    <AtollLogo
      size={size}
      activity={display.activity}
      idleIntervalSec={idleIntervalSec}
      idleDurationSec={idleDurationSec}
      motionPaused={motionPaused}
      reaction={reaction}
      reactionKey={reactionKey}
    />
  );
}
