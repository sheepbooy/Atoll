import { useMascotMotion, type MascotMotionProps } from "./mascotMotion";
import type { ClawdMood } from "./ClawdMascot";
import { brandMascotViewBox, MascotExtras, MascotShadow, mascotSizeStyle } from "./mascotShared";
import { GeminiOfficialMark } from "./officialMarks";

interface GeminiMascotProps extends MascotMotionProps {
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
  ...motionProps
}: GeminiMascotProps) {
  const motion = useMascotMotion({ mood, animated, size, character: "gemini", ...motionProps });
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
      ref={motion.ref}
      className={`gemini is-${mood} ${motion.motionClassName}${animated ? "" : " is-static"}${className ? ` ${className}` : ""}`}
      style={{ ...mascotSizeStyle(size), ...motion.motionStyle }}
      aria-hidden="true"
    >
      <svg
        className="gemini-svg"
        width="100%"
        height="100%"
        viewBox={brandMascotViewBox(size, "gemini")}
        preserveAspectRatio="xMidYMid meet"
      >
        <MascotShadow className="gemini-shadow" />

        <g className="gemini-body" data-part="body">
          <GeminiOfficialMark
            mood={mood}
            accent={accent}
            accentDark={accentDark}
            className="gemini-mark"
            timeline={motion.timeline}
          />
        </g>
        <g className="mascot-extras">
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
