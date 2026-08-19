import type { ClawdMood } from "./ClawdMascot";
import {
  CURSOR_FACE,
  DEAD_BODY,
  DEAD_BODY_TOP,
  DEAD_DARK,
  DEAD_EYE,
  EYE,
  MASCOT_VIEWBOX_ATTR,
  MascotExtras,
  MascotFace,
  MascotShadow,
  SICK,
  SICK_DARK,
  mascotSizeStyle,
  mixHex,
  useMascotBlink,
} from "./mascotShared";

const FRONT = "#a78bfa";
const TOP = "#c4b5fd";
const SIDE = "#7c5fd4";
const BLUSH = "#f0e8ff";

interface CursorPalette {
  front: string;
  top: string;
  side: string;
  blush: string;
  sparkle: string;
  sweat: string;
}

function defaultCubePalette(): CursorPalette {
  return {
    front: FRONT,
    top: TOP,
    side: SIDE,
    blush: BLUSH,
    sparkle: "#f5f3ff",
    sweat: "#c4b5fd",
  };
}

const deadCubePalette: CursorPalette = {
  front: DEAD_BODY,
  top: DEAD_BODY_TOP,
  side: DEAD_DARK,
  blush: "#b0b4b8",
  sparkle: "#c8ccd0",
  sweat: "#a0a4a8",
};

function deriveCubePalette(accent?: string, accentDark?: string): CursorPalette {
  if (!accent) return defaultCubePalette();
  const side = accentDark ?? mixHex(accent, "#000000", 0.28);
  return {
    front: accent,
    top: mixHex(accent, "#ffffff", 0.28),
    side,
    blush: mixHex(accent, "#ffffff", 0.45),
    sparkle: mixHex(accent, "#ffffff", 0.62),
    sweat: mixHex(accent, "#ffffff", 0.35),
  };
}

function sickCubePalette(): CursorPalette {
  return {
    front: SICK,
    top: mixHex(SICK, "#ffffff", 0.18),
    side: SICK_DARK,
    blush: "#d4f5d4",
    sparkle: "#e8ffe8",
    sweat: "#a8d8a8",
  };
}

function PixelPointer({ fill }: { fill: string }) {
  return (
    <g className="cursor-mascot-claw cursor-mascot-claw-left cursor-mascot-pointer">
      <rect x={16} y={-14} width={8} height={8} fill={fill} />
      <rect x={16} y={-6} width={16} height={8} fill={fill} />
      <rect x={16} y={2} width={8} height={12} fill={fill} />
      <rect x={24} y={2} width={8} height={8} fill={fill} />
    </g>
  );
}

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
  accent,
  accentDark,
  animated = true,
}: CursorMascotProps) {
  const blinking = useMascotBlink(animated, mood);
  const palette =
    mood === "dead"
      ? deadCubePalette
      : mood === "worried"
        ? sickCubePalette()
        : deriveCubePalette(accent, accentDark);

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
        shapeRendering="crispEdges"
      >
        <MascotShadow className="cursor-mascot-shadow" />

        <g className="cursor-mascot-body">
          <PixelPointer fill={palette.front} />
          <rect
            className="cursor-mascot-claw cursor-mascot-claw-right"
            x={100}
            y={28}
            width={10}
            height={14}
            fill={palette.side}
          />

          <g className="cursor-mascot-cube">
            <rect
              className="cursor-mascot-face-front"
              x={28}
              y={6}
              width={56}
              height={56}
              fill={palette.front}
            />
            <rect
              className="cursor-mascot-face-top"
              x={28}
              y={6}
              width={56}
              height={10}
              fill={palette.top}
            />
            <rect
              className="cursor-mascot-face-side"
              x={84}
              y={14}
              width={16}
              height={48}
              fill={palette.side}
            />
          </g>

          <MascotFace
            mood={mood}
            blinking={blinking}
            eyeFill={mood === "dead" ? DEAD_EYE : EYE}
            blushFill={palette.blush}
            layout={CURSOR_FACE}
          />

          <rect className="cursor-mascot-leg cursor-mascot-leg-0" x={38} y={62} width={12} height={16} fill={palette.side} />
          <rect className="cursor-mascot-leg cursor-mascot-leg-1" x={66} y={62} width={12} height={16} fill={palette.side} />

          <MascotExtras
            mood={mood}
            classPrefix="cursor-mascot"
            heartFill={palette.sparkle}
            sparkle={palette.sparkle}
            sweat={palette.sweat}
          />
        </g>
      </svg>
    </span>
  );
}
