import { useMascotMotion, type MascotMotionProps } from "./mascotMotion";
import type { ClawdMood } from "./ClawdMascot";
import { brandMascotViewBox, MascotExtras, MascotShadow, mascotSizeStyle } from "./mascotShared";
import { CodexOfficialMark, codexMarkFill } from "./officialMarks";

interface CodexMascotProps extends MascotMotionProps {
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
  ...motionProps
}: CodexMascotProps) {
  const motion = useMascotMotion({ mood, animated, size, character: "codex", ...motionProps });
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
      ref={motion.ref}
      className={`codex is-${mood} ${motion.motionClassName}${animated ? "" : " is-static"}${className ? ` ${className}` : ""}`}
      style={{ ...mascotSizeStyle(size), ...motion.motionStyle }}
      aria-hidden="true"
    >
      <svg
        className="codex-svg"
        width="100%"
        height="100%"
        viewBox={brandMascotViewBox(size, "codex")}
        preserveAspectRatio="xMidYMid meet"
      >
        <MascotShadow className="codex-shadow" />

        <g className="codex-body" data-part="body">
          <CodexOfficialMark className="codex-mark" fill={fill} />
        </g>
        <g className="mascot-extras">
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
