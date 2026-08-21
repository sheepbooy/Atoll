import type { ClawdMood } from "./ClawdMascot";
import { MASCOT_VIEWBOX_ATTR, MascotExtras, MascotShadow, mascotSizeStyle } from "./mascotShared";
import { CursorOfficialMark, cursorCubePalette } from "./officialMarks";

interface CursorMascotProps {
  mood: ClawdMood;
  size?: number;
  className?: string;
  accent?: string;
  accentDark?: string;
  animated?: boolean;
}

export function CursorMascot({
  mood,
  size,
  className,
  animated = true,
}: CursorMascotProps) {
  const palette = cursorCubePalette(mood);
  const extras =
    mood === "dead"
      ? { sparkle: "#c8ccd0", sweat: "#a0a4a8" }
      : mood === "worried"
        ? { sparkle: "#e8ffe8", sweat: "#a8d8a8" }
        : { sparkle: "#f5f3ff", sweat: "#c4b5fd" };

  return (
    <span
      className={`cursor-mascot is-${mood}${animated ? "" : " is-static"}${className ? ` ${className}` : ""}`}
      style={mascotSizeStyle(size)}
      aria-hidden="true"
    >
      <svg
        className="cursor-mascot-svg"
        width="100%"
        height="100%"
        viewBox={MASCOT_VIEWBOX_ATTR}
        preserveAspectRatio="xMidYMid meet"
      >
        <MascotShadow className="cursor-mascot-shadow" />

        <g className="cursor-mascot-body">
          <CursorOfficialMark className="cursor-mascot-cube cursor-mascot-mark" palette={palette} />
          <MascotExtras
            mood={mood}
            classPrefix="cursor-mascot"
            heartFill={extras.sparkle}
            sparkle={extras.sparkle}
            sweat={extras.sweat}
          />
        </g>
      </svg>
    </span>
  );
}
