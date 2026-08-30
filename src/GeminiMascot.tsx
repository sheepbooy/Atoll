import type { ClawdMood } from "./ClawdMascot";
import { MASCOT_VIEWBOX_ATTR, MascotExtras, MascotShadow, mascotSizeStyle } from "./mascotShared";
import { GeminiOfficialMark } from "./officialMarks";

interface GeminiMascotProps {
  mood: ClawdMood;
  size?: number;
  className?: string;
  accent?: string;
  accentDark?: string;
  animated?: boolean;
}

export function GeminiMascot({
  mood,
  size,
  className,
  accent,
  accentDark,
  animated = true,
}: GeminiMascotProps) {
  const extras =
    mood === "dead"
      ? { sparkle: "#b0b8bc", sweat: "#9aa0a4" }
      : mood === "worried"
        ? { sparkle: "#e5f4ff", sweat: "#9cc8e8" }
        : {
            sparkle: accent || "#d0dcff",
            sweat: accentDark || accent || "#8f7bc7",
          };

  return (
    <span
      className={`gemini is-${mood}${animated ? "" : " is-static"}${className ? ` ${className}` : ""}`}
      style={mascotSizeStyle(size)}
      aria-hidden="true"
    >
      <svg
        className="gemini-svg"
        width="100%"
        height="100%"
        viewBox={MASCOT_VIEWBOX_ATTR}
        preserveAspectRatio="xMidYMid meet"
      >
        <MascotShadow className="gemini-shadow" />

        <g className="gemini-body">
          <GeminiOfficialMark
            mood={mood}
            accent={accent}
            accentDark={accentDark}
            className="gemini-mark"
          />
          <MascotExtras
            mood={mood}
            classPrefix="gemini"
            heartFill={extras.sparkle}
            sparkle={extras.sparkle}
            sweat={extras.sweat}
          />
        </g>
      </svg>
    </span>
  );
}
