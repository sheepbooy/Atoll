import type { ClawdMood } from "./ClawdMascot";
import { MASCOT_VIEWBOX_ATTR, MascotExtras, MascotShadow, mascotSizeStyle } from "./mascotShared";
import { CodexOfficialMark, codexMarkFill } from "./officialMarks";

interface CodexMascotProps {
  mood: ClawdMood;
  size?: number;
  className?: string;
  accent?: string;
  accentDark?: string;
  animated?: boolean;
}

export function CodexMascot({
  mood,
  size,
  className,
  accent,
  accentDark,
  animated = true,
}: CodexMascotProps) {
  const fill = codexMarkFill(mood, accent);
  const extras =
    mood === "dead"
      ? { sparkle: "#b0b8bc", sweat: "#9aa0a4" }
      : mood === "worried"
        ? { sparkle: "#e8ffe8", sweat: "#a8d8a8" }
        : {
            sparkle: accent || "#d4f8ff",
            sweat: accentDark || accent || "#7cc4ff",
          };

  return (
    <span
      className={`codex is-${mood}${animated ? "" : " is-static"}${className ? ` ${className}` : ""}`}
      style={mascotSizeStyle(size)}
      aria-hidden="true"
    >
      <svg
        className="codex-svg"
        width="100%"
        height="100%"
        viewBox={MASCOT_VIEWBOX_ATTR}
        preserveAspectRatio="xMidYMid meet"
      >
        <MascotShadow className="codex-shadow" />

        <g className="codex-body">
          <CodexOfficialMark className="codex-mark" fill={fill} />
          <MascotExtras
            mood={mood}
            classPrefix="codex"
            heartFill={extras.sparkle}
            sparkle={extras.sparkle}
            sweat={extras.sweat}
          />
        </g>
      </svg>
    </span>
  );
}
