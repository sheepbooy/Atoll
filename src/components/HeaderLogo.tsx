import {
  type HeaderLogoDisplay,
} from "../hookHealth";
import {
  AgentMascot,
} from "../AgentMascot";
import {
  AtollLogo,
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
}

export function HeaderLogo({
  display,
  size,
  idleIntervalSec,
  idleDurationSec,
  motionPaused,
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
    />
  );
}
