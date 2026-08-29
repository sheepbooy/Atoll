import type { ClawdMood } from "./ClawdMascot";
import { MASCOT_VIEWBOX_ATTR, MascotExtras, MascotShadow, mascotSizeStyle } from "./mascotShared";
import { ZcodeOfficialMark } from "./officialMarks";

interface ZcodeMascotProps {
  mood: ClawdMood;
  size?: number;
  className?: string;
  accent?: string;
  accentDark?: string;
  animated?: boolean;
}

export function ZcodeMascot({
  mood,
  size,
  className,
  accent,
  accentDark,
  animated = true,
}: ZcodeMascotProps) {
  const extras =
    mood === "dead"
      ? { sparkle: "#b0b8bc", sweat: "#9aa0a4" }
      : mood === "worried"
        ? { sparkle: "#e5f4ff", sweat: "#9cc8e8" }
        : {
            sparkle: accent || "#d4ecff",
            sweat: accentDark || accent || "#7cb8e8",
          };

  return (
    <span
      className={`zcode is-${mood}${animated ? "" : " is-static"}${className ? ` ${className}` : ""}`}
      style={mascotSizeStyle(size)}
      aria-hidden="true"
    >
      <svg
        className="zcode-svg"
        width="100%"
        height="100%"
        viewBox={MASCOT_VIEWBOX_ATTR}
        preserveAspectRatio="xMidYMid meet"
      >
        <MascotShadow className="zcode-shadow" />

        <g className="zcode-body">
          <ZcodeOfficialMark mood={mood} className="zcode-mark" />
          <MascotExtras
            mood={mood}
            classPrefix="zcode"
            heartFill={extras.sparkle}
            sparkle={extras.sparkle}
            sweat={extras.sweat}
          />
        </g>
      </svg>
    </span>
  );
}
