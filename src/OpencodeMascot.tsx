import type { ClawdMood } from "./ClawdMascot";
import { MASCOT_VIEWBOX_ATTR, MascotExtras, MascotShadow, mascotSizeStyle } from "./mascotShared";
import { OpencodeOfficialMark } from "./officialMarks";

interface OpencodeMascotProps {
  mood: ClawdMood;
  size?: number;
  className?: string;
  accent?: string;
  accentDark?: string;
  animated?: boolean;
}

export function OpencodeMascot({
  mood,
  size,
  className,
  accent,
  accentDark,
  animated = true,
}: OpencodeMascotProps) {
  const extras =
    mood === "dead"
      ? { sparkle: "#b0b8bc", sweat: "#9aa0a4" }
      : mood === "worried"
        ? { sparkle: "#e2f6f1", sweat: "#70d8c8" }
        : {
            sparkle: accent || "#d4f5ee",
            sweat: accentDark || accent || "#2aa896",
          };

  return (
    <span
      className={`opencode is-${mood}${animated ? "" : " is-static"}${className ? ` ${className}` : ""}`}
      style={mascotSizeStyle(size)}
      aria-hidden="true"
    >
      <svg
        className="opencode-svg"
        width="100%"
        height="100%"
        viewBox={MASCOT_VIEWBOX_ATTR}
        preserveAspectRatio="xMidYMid meet"
      >
        <MascotShadow className="opencode-shadow" />

        <g className="opencode-body">
          <OpencodeOfficialMark
            mood={mood}
            accent={accent}
            accentDark={accentDark}
            className="opencode-mark"
          />
          <MascotExtras
            mood={mood}
            classPrefix="opencode"
            heartFill={extras.sparkle}
            sparkle={extras.sparkle}
            sweat={extras.sweat}
          />
        </g>
      </svg>
    </span>
  );
}
