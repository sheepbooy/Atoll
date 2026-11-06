import { useMascotMotion, type MascotMotionProps } from "./mascotMotion";
import type { ClawdMood } from "./ClawdMascot";
import { brandMascotViewBox, MascotExtras, MascotShadow, mascotSizeStyle } from "./mascotShared";
import { PiOfficialMark } from "./officialMarks";

interface PiMascotProps extends MascotMotionProps {
  mood: ClawdMood;
  size?: number;
  className?: string;
  accent?: string;
  accentDark?: string;
  animated?: boolean;
}

export function PiMascot({
  mood,
  size,
  className,
  accent,
  accentDark,
  animated = true,
  ...motionProps
}: PiMascotProps) {
  const motion = useMascotMotion({ mood, animated, size, character: "pi", ...motionProps });
  const extras =
    mood === "dead"
      ? { sparkle: "#b0b8bc", sweat: "#9aa0a4" }
      : mood === "worried"
        ? { sparkle: "#dbeaf2", sweat: "#4d9abf" }
        : {
            sparkle: accent || "#dbeaf2",
            sweat: accentDark || accent || "#3d7d9c",
          };

  return (
    <span
      ref={motion.ref}
      className={`pi is-${mood} ${motion.motionClassName}${animated ? "" : " is-static"}${className ? ` ${className}` : ""}`}
      style={{ ...mascotSizeStyle(size), ...motion.motionStyle }}
      aria-hidden="true"
    >
      <svg
        className="pi-svg"
        width="100%"
        height="100%"
        viewBox={brandMascotViewBox(size, "pi")}
        preserveAspectRatio="xMidYMid meet"
      >
        <MascotShadow className="pi-shadow" />

        <g className="pi-body" data-part="body">
          <PiOfficialMark
            mood={mood}
            accent={accent}
            accentDark={accentDark}
            className="pi-mark"
          />
        </g>
        <g className="mascot-extras">
          <MascotExtras
            mood={mood}
            classPrefix="pi"
            heartFill={extras.sparkle}
            sparkle={extras.sparkle}
            sweat={extras.sweat}
          />
        </g>
      </svg>
    </span>
  );
}
