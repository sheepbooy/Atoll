import { useMascotMotion, type MascotMotionProps } from "./mascotMotion";
import type { ClawdMood } from "./ClawdMascot";
import { brandMascotViewBox, MascotExtras, MascotShadow, mascotSizeStyle } from "./mascotShared";
import { DshOfficialMark } from "./officialMarks";

interface DshMascotProps extends MascotMotionProps {
  mood: ClawdMood;
  size?: number;
  className?: string;
  accent?: string;
  accentDark?: string;
  animated?: boolean;
}

export function DshMascot({
  mood,
  size,
  className,
  accent,
  accentDark,
  animated = true,
  ...motionProps
}: DshMascotProps) {
  const motion = useMascotMotion({ mood, animated, size, character: "dsh", ...motionProps });
  const extras =
    mood === "dead"
      ? { sparkle: "#b0b8bc", sweat: "#9aa0a4" }
      : mood === "worried"
        ? { sparkle: "#e5f4ff", sweat: "#9cc8e8" }
        : {
            sparkle: accent || "#b8c4ff",
            sweat: accentDark || accent || "#6f86ff",
          };

  return (
    <span
      ref={motion.ref}
      className={`dsh is-${mood} ${motion.motionClassName}${animated ? "" : " is-static"}${className ? ` ${className}` : ""}`}
      style={{ ...mascotSizeStyle(size), ...motion.motionStyle }}
      aria-hidden="true"
    >
      <svg
        className="dsh-svg"
        width="100%"
        height="100%"
        viewBox={brandMascotViewBox(size, "dsh")}
        preserveAspectRatio="xMidYMid meet"
      >
        <MascotShadow className="dsh-shadow" />

        <g className="dsh-mark-wrap">
          <DshOfficialMark
            mood={mood}
            accent={accent}
            accentDark={accentDark}
            className="dsh-mark"
          />
        </g>
        <g className="mascot-extras">
          <MascotExtras
            mood={mood}
            classPrefix="dsh"
            heartFill={extras.sparkle}
            sparkle={extras.sparkle}
            sweat={extras.sweat}
          />
        </g>
      </svg>
    </span>
  );
}
