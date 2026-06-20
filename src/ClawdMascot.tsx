import { useEffect, useState } from "react";

export type ClawdMood = "sleeping" | "calm" | "alert" | "worried" | "happy" | "sad" | "dead";

const BODY = "#c27c5c";
const BODY_TOP = "#d08a68";
const DARK = "#8b5a42";
const DEAD_BODY = "#8a8a8a";
const DEAD_BODY_TOP = "#9a9a9a";
const DEAD_DARK = "#666666";
const EYE = "#1a1a1a";
const DEAD_EYE = "#050505";
const SICK = "#7cb97c";
const SICK_DARK = "#5a8b5a";
const BLUSH = "#ffb4b4";

const LEGS = [16, 30.4, 72, 86.4];

const VIEWBOX = { x: -20, y: -34, w: 152, h: 136 };
const ASPECT = VIEWBOX.w / VIEWBOX.h;

interface ClawdMascotProps {
  mood: ClawdMood;
  size?: number;
  className?: string;
  accent?: string;
  accentDark?: string;
}

export function ClawdMascot({
  mood,
  size,
  className,
  accent,
  accentDark,
}: ClawdMascotProps) {
  const [blinking, setBlinking] = useState(false);

  useEffect(() => {
    if (mood === "sleeping" || mood === "dead") return;
    let timer: number;
    const loop = () => {
      setBlinking(true);
      window.setTimeout(() => setBlinking(false), 150);
      timer = window.setTimeout(loop, 3000 + Math.random() * 2500);
    };
    timer = window.setTimeout(loop, 2500 + Math.random() * 2500);
    return () => window.clearTimeout(timer);
  }, [mood]);

  const isDead = mood === "dead";
  const isSick = mood === "worried";
  const body = isDead ? DEAD_BODY : isSick ? SICK : accent ?? BODY;
  const bodyTop = isDead ? DEAD_BODY_TOP : isSick ? SICK : accent ?? BODY_TOP;
  const dark = isDead ? DEAD_DARK : isSick ? SICK_DARK : accentDark ?? DARK;
  const eyeHeight = blinking ? 2.4 : 16;

  const wrapperStyle = size
    ? { width: size * ASPECT, height: size }
    : undefined;

  return (
    <span
      className={`clawd is-${mood}${className ? ` ${className}` : ""}`}
      style={wrapperStyle}
      aria-hidden="true"
    >
      <svg
        className="clawd-svg"
        width="100%"
        height="100%"
        viewBox={`${VIEWBOX.x} ${VIEWBOX.y} ${VIEWBOX.w} ${VIEWBOX.h}`}
        preserveAspectRatio="xMidYMid meet"
        shapeRendering="crispEdges"
      >
        <ellipse className="clawd-shadow" cx={68} cy={92} rx={32} ry={4} fill="rgba(0,0,0,0.18)" />

        <g className="clawd-body">
          <rect className="clawd-claw clawd-claw-left" x={-4} y={25.6} width={12} height={14.4} fill={body} />
          <rect className="clawd-claw clawd-claw-right" x={104} y={25.6} width={12} height={14.4} fill={body} />

          <rect x={8} y={0} width={96} height={56} fill={body} />
          <rect x={8} y={0} width={96} height={9} fill={bodyTop} />

          {mood === "dead" ? (
            <>
              <line x1={26} y1={10} x2={38} y2={30} stroke={DEAD_EYE} strokeWidth={3.2} strokeLinecap="round" />
              <line x1={38} y1={10} x2={26} y2={30} stroke={DEAD_EYE} strokeWidth={3.2} strokeLinecap="round" />
              <line x1={74} y1={10} x2={86} y2={30} stroke={DEAD_EYE} strokeWidth={3.2} strokeLinecap="round" />
              <line x1={86} y1={10} x2={74} y2={30} stroke={DEAD_EYE} strokeWidth={3.2} strokeLinecap="round" />
            </>
          ) : isSick ? (
            <>
              <line x1={28} y1={12} x2={36} y2={28} stroke={EYE} strokeWidth={2.4} strokeLinecap="round" />
              <line x1={36} y1={12} x2={28} y2={28} stroke={EYE} strokeWidth={2.4} strokeLinecap="round" />
              <line x1={76} y1={12} x2={84} y2={28} stroke={EYE} strokeWidth={2.4} strokeLinecap="round" />
              <line x1={84} y1={12} x2={76} y2={28} stroke={EYE} strokeWidth={2.4} strokeLinecap="round" />
            </>
          ) : mood === "sleeping" ? (
            <>
              <rect x={28} y={16} width={9.6} height={2.4} fill={EYE} />
              <rect x={74.4} y={16} width={9.6} height={2.4} fill={EYE} />
            </>
          ) : (
            <>
              <rect x={28} y={12} width={8} height={eyeHeight} fill={EYE} />
              <rect x={76} y={12} width={8} height={eyeHeight} fill={EYE} />
            </>
          )}

          {mood === "sad" && (
            <>
              <rect x={24} y={8} width={12} height={2.4} fill={EYE} transform="rotate(-15 30 9.2)" />
              <rect x={76} y={8} width={12} height={2.4} fill={EYE} transform="rotate(15 82 9.2)" />
            </>
          )}

          {(mood === "happy" || mood === "alert") && (
            <>
              <ellipse cx={22} cy={42} rx={6} ry={3.2} fill={BLUSH} opacity={0.65} />
              <ellipse cx={90} cy={42} rx={6} ry={3.2} fill={BLUSH} opacity={0.65} />
            </>
          )}

          {LEGS.map((x, i) => (
            <rect
              key={x}
              className={`clawd-leg clawd-leg-${i}`}
              x={x}
              y={56}
              width={9.6}
              height={20}
              fill={dark}
            />
          ))}
        </g>

        {mood === "alert" && (
          <g className="clawd-bang">
            <rect x={53} y={-30} width={6} height={14} rx={1.5} fill="#f8dda0" />
            <rect x={53} y={-13} width={6} height={5} rx={1.5} fill="#f8dda0" />
          </g>
        )}

        {mood === "happy" && (
          <>
            <path
              className="clawd-heart"
              d="M56 -14 C 51 -22 41 -17 56 -4 C 71 -17 61 -22 56 -14 Z"
              fill="#ff8d8d"
            />
            <g transform="translate(-6 12)">
              <Star className="clawd-star clawd-star-0" />
            </g>
            <g transform="translate(118 6)">
              <Star className="clawd-star clawd-star-1" />
            </g>
            <g transform="translate(96 -24)">
              <Star className="clawd-star clawd-star-2" />
            </g>
          </>
        )}

        {mood === "sleeping" && (
          <g className="clawd-zzz" fill="#aab4ff" fontFamily="var(--font-mono, monospace)" fontWeight="700">
            <text className="clawd-z clawd-z-0" x={104} y={-6} fontSize={16}>z</text>
            <text className="clawd-z clawd-z-1" x={116} y={-16} fontSize={20}>z</text>
            <text className="clawd-z clawd-z-2" x={128} y={-28} fontSize={24}>z</text>
          </g>
        )}

        {mood === "worried" && (
          <g transform="translate(108 -4)">
            <path className="clawd-sweat" d="M4 0 C 8 7 0 7 4 0 Z" fill="#7cc4ff" />
          </g>
        )}
      </svg>
    </span>
  );
}

function Star({ className }: { className?: string }) {
  return (
    <polygon
      className={className}
      points="0,-6 1.6,-1.6 6,0 1.6,1.6 0,6 -1.6,1.6 -6,0 -1.6,-1.6"
      fill="#ffe6a8"
    />
  );
}
