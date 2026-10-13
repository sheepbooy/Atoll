import { useMascotMotion, type MascotMotionProps } from "./mascotMotion";
import type { ClawdMood } from "./ClawdMascot";
import { brandMascotViewBox, MascotExtras, MascotShadow, mascotSizeStyle } from "./mascotShared";
import { ZcodeOfficialMark } from "./officialMarks";

interface ZcodeMascotProps extends MascotMotionProps {
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
  ...motionProps
}: ZcodeMascotProps) {
  const motion = useMascotMotion({ mood, animated, size, character: "zcode", ...motionProps });
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
      ref={motion.ref}
      className={`zcode is-${mood} ${motion.motionClassName}${animated ? "" : " is-static"}${className ? ` ${className}` : ""}`}
      style={{ ...mascotSizeStyle(size), ...motion.motionStyle }}
      aria-hidden="true"
    >
      <svg
        className="zcode-svg"
        width="100%"
        height="100%"
        viewBox={brandMascotViewBox(size, "zcode")}
        preserveAspectRatio="xMidYMid meet"
      >
        <MascotShadow className="zcode-shadow" />

        <g className="zcode-body" data-part="body">
          <ZcodeOfficialMark
            mood={mood}
            accent={accent}
            accentDark={accentDark}
            className="zcode-mark"
          />
        </g>
        <g className="mascot-extras">
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
