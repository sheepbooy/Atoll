import { useMascotMotion, type MascotMotionProps } from "./mascotMotion";
import type { ClawdMood } from "./ClawdMascot";
import { brandMascotViewBox, MascotExtras, MascotShadow, mascotSizeStyle } from "./mascotShared";
import { OpencodeOfficialMark } from "./officialMarks";

interface OpencodeMascotProps extends MascotMotionProps {
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
  ...motionProps
}: OpencodeMascotProps) {
  const motion = useMascotMotion({ mood, animated, size, character: "opencode", ...motionProps });
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
      ref={motion.ref}
      className={`opencode is-${mood} ${motion.motionClassName}${animated ? "" : " is-static"}${className ? ` ${className}` : ""}`}
      style={{ ...mascotSizeStyle(size), ...motion.motionStyle }}
      aria-hidden="true"
    >
      <svg
        className="opencode-svg"
        width="100%"
        height="100%"
        viewBox={brandMascotViewBox(size, "opencode")}
        preserveAspectRatio="xMidYMid meet"
      >
        <MascotShadow className="opencode-shadow" />

        <g className="opencode-body" data-part="body">
          <OpencodeOfficialMark
            mood={mood}
            accent={accent}
            accentDark={accentDark}
            className="opencode-mark"
          />
        </g>
        <g className="mascot-extras">
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
