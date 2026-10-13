import { useMascotMotion, type MascotMotionProps } from "./mascotMotion";
import {
  BLUSH,
  DEAD_BODY,
  DEAD_BODY_TOP,
  DEAD_DARK,
  DEAD_EYE,
  EYE,
  MASCOT_BODY,
  MASCOT_VIEWBOX_ATTR,
  MascotClaws,
  MascotExtras,
  MascotFace,
  MascotLegs,
  MascotShadow,
  SICK,
  SICK_DARK,
  mascotSizeStyle,
  useMascotBlink,
  type ClawdMood,
} from "./mascotShared";

export type { ClawdMood };

const BODY = "#c27c5c";
const BODY_TOP = "#d08a68";
const DARK = "#8b5a42";

interface ClawdMascotProps extends MascotMotionProps {
  mood: ClawdMood;
  size?: number;
  className?: string;
  accent?: string;
  accentDark?: string;
  animated?: boolean;
}

export function ClawdMascot({
  mood,
  size,
  className,
  accent,
  accentDark,
  animated = true,
  ...motionProps
}: ClawdMascotProps) {
  const motion = useMascotMotion({ mood, animated, size, ...motionProps });
  const blinking = useMascotBlink(motion.ambientEnabled, mood);
  const isDead = mood === "dead";
  const isSick = mood === "worried";
  const body = isDead ? DEAD_BODY : isSick ? SICK : accent ?? BODY;
  const bodyTop = isDead ? DEAD_BODY_TOP : isSick ? SICK : accent ?? BODY_TOP;
  const dark = isDead ? DEAD_DARK : isSick ? SICK_DARK : accentDark ?? DARK;

  return (
    <span
      ref={motion.ref}
      className={`clawd is-${mood} ${motion.motionClassName}${animated ? "" : " is-static"}${className ? ` ${className}` : ""}`}
      style={{ ...mascotSizeStyle(size), ...motion.motionStyle }}
      aria-hidden="true"
    >
      <svg
        className="clawd-svg"
        width="100%"
        height="100%"
        viewBox={MASCOT_VIEWBOX_ATTR}
        preserveAspectRatio="xMidYMid meet"
        shapeRendering="crispEdges"
      >
        <MascotShadow className="clawd-shadow" />

        <g className="clawd-body">
          <rect x={MASCOT_BODY.x} y={MASCOT_BODY.y} width={MASCOT_BODY.w} height={MASCOT_BODY.h} fill={body} />
          <rect x={MASCOT_BODY.x} y={MASCOT_BODY.y} width={MASCOT_BODY.w} height={MASCOT_BODY.topH} fill={bodyTop} />
          <MascotClaws classPrefix="clawd" fill={body} />
          <g className="clawd-eyes" data-part="eyes">
            <MascotFace
              mood={mood}
              blinking={blinking}
              eyeFill={isDead ? DEAD_EYE : EYE}
              blushFill={BLUSH}
            />
          </g>
          <MascotLegs classPrefix="clawd" fill={dark} />
        </g>
        <g className="mascot-extras">
          <MascotExtras mood={mood} classPrefix="clawd" />
        </g>
      </svg>
    </span>
  );
}
