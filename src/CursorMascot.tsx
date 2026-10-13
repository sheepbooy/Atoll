import { useMascotMotion, type MascotMotionProps } from "./mascotMotion";
import type { CSSProperties } from "react";
import type { ClawdMood } from "./ClawdMascot";
import { brandMascotViewBox, MascotExtras, MascotShadow, mascotSizeStyle } from "./mascotShared";
import { CursorOfficialMark, cursorCubePalette } from "./officialMarks";

const CURSOR_OUTLINE_FALLBACK = "#f4f0e8";

interface CursorMascotProps extends MascotMotionProps {
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
  accent,
  animated = true,
  ...motionProps
}: CursorMascotProps) {
  const motion = useMascotMotion({ mood, animated, size, character: "cursor", ...motionProps });
  const palette = cursorCubePalette(mood);
  const extras =
    mood === "dead"
      ? { sparkle: "#c8ccd0", sweat: "#a0a4a8" }
      : mood === "worried"
        ? { sparkle: "#e8ffe8", sweat: "#a8d8a8" }
        : { sparkle: "#f5f3ff", sweat: "#c4b5fd" };
  const outline = accent || CURSOR_OUTLINE_FALLBACK;
  const style = {
    ...mascotSizeStyle(size),
    ...motion.motionStyle,
    "--cursor-outline": outline,
    color: outline,
  } as CSSProperties;

  return (
    <span
      ref={motion.ref}
      className={`cursor-mascot is-${mood} ${motion.motionClassName}${animated ? "" : " is-static"}${className ? ` ${className}` : ""}`}
      style={style}
      aria-hidden="true"
    >
      <svg
        className="cursor-mascot-svg"
        width="100%"
        height="100%"
        viewBox={brandMascotViewBox(size, "cursor")}
        preserveAspectRatio="xMidYMid meet"
      >
        <MascotShadow className="cursor-mascot-shadow" />

        <g className="cursor-mascot-body" data-part="body">
          <CursorOfficialMark className="cursor-mascot-cube cursor-mascot-mark" palette={palette} />
        </g>
        <g className="mascot-extras">
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
