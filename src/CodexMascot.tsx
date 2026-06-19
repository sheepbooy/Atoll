import type { ClawdMood } from "./ClawdMascot";

const BODY = "#4a9fd4";
const BODY_TOP = "#6eb8e6";
const DARK = "#3589b8";
const SCREEN = "#0c1018";
const BEZEL = "#2d6d96";
const PROMPT = "#3de8f8";
const PROMPT_DIM = "#2a9aaa";
const SICK = "#7cb97c";
const SICK_DARK = "#5a8b5a";
const BLUSH = "#b8ecff";
const OUTLINE = "#9ed8f4";
const OUTLINE_SICK = "#d4f5d4";
const BEZEL_RIM = "#5ab4dc";

const MONO = "ui-monospace, 'SF Mono', Menlo, monospace";

const VIEWBOX = { x: -20, y: -34, w: 152, h: 136 };
const ASPECT = VIEWBOX.w / VIEWBOX.h;

/** Screen center — prompt text anchor */
const FACE_CX = 60;
const FACE_CY = 30;
const PROMPT_SIZE = 38;
const RIM = 2.5;

function RimRect({
  x,
  y,
  width,
  height,
  fill,
  stroke,
  className,
}: {
  x: number;
  y: number;
  width: number;
  height: number;
  fill: string;
  stroke: string;
  className?: string;
}) {
  return (
    <rect
      className={className}
      x={x}
      y={y}
      width={width}
      height={height}
      fill={fill}
      stroke={stroke}
      strokeWidth={RIM}
    />
  );
}

interface CodexMascotProps {
  mood: ClawdMood;
  size?: number;
  className?: string;
  accent?: string;
  accentDark?: string;
}

function ScreenPrompt({
  mood,
  color,
}: {
  mood: ClawdMood;
  color: string;
}) {
  const base = {
    x: FACE_CX,
    y: FACE_CY,
    textAnchor: "middle" as const,
    dominantBaseline: "central" as const,
    fontFamily: MONO,
    fontSize: PROMPT_SIZE,
    fontWeight: 700,
  };

  if (mood === "sleeping") {
    return (
      <text {...base} fill={color} opacity={0.45}>
        --
      </text>
    );
  }

  if (mood === "worried") {
    return (
      <text {...base} fill={color}>
        xx
      </text>
    );
  }

  const opacity = mood === "sad" ? 0.6 : 1;

  return (
    <text {...base} fill={color} opacity={opacity}>
      <tspan>&gt;</tspan>
      <tspan className="codex-cursor">_</tspan>
    </text>
  );
}

export function CodexMascot({
  mood,
  size,
  className,
  accent,
  accentDark,
}: CodexMascotProps) {
  const isSick = mood === "worried";
  const body = isSick ? SICK : accent ?? BODY;
  const bodyTop = isSick ? SICK : accent ?? BODY_TOP;
  const dark = isSick ? SICK_DARK : accentDark ?? DARK;
  const rim = isSick ? OUTLINE_SICK : OUTLINE;
  const prompt = isSick
    ? "#e8ffe8"
    : mood === "sleeping"
      ? PROMPT_DIM
      : PROMPT;

  const wrapperStyle = size
    ? { width: size * ASPECT, height: size }
    : undefined;

  return (
    <span
      className={`codex is-${mood}${className ? ` ${className}` : ""}`}
      style={wrapperStyle}
      aria-hidden="true"
    >
      <svg
        className="codex-svg"
        width="100%"
        height="100%"
        viewBox={`${VIEWBOX.x} ${VIEWBOX.y} ${VIEWBOX.w} ${VIEWBOX.h}`}
        preserveAspectRatio="xMidYMid meet"
      >
        <ellipse
          className="codex-shadow"
          cx={68}
          cy={92}
          rx={34}
          ry={5}
          fill="rgba(0,0,0,0.32)"
        />

        <g className="codex-body" shapeRendering="crispEdges">
          <RimRect x={38} y={-5} width={32} height={6} fill={body} stroke={rim} />

          <RimRect x={16} y={0} width={88} height={56} fill={body} stroke={rim} />
          <rect x={16} y={0} width={88} height={6} fill={bodyTop} />

          <RimRect
            className="codex-screen"
            x={18}
            y={5}
            width={84}
            height={49}
            fill={BEZEL}
            stroke={BEZEL_RIM}
          />
          <rect x={20} y={7} width={80} height={45} fill={SCREEN} />

          <ScreenPrompt mood={mood} color={prompt} />

          {mood === "sad" && (
            <>
              <rect
                x={20}
                y={2}
                width={12}
                height={2.4}
                fill={SCREEN}
                stroke={rim}
                strokeWidth={1.2}
                transform="rotate(-14 26 3.2)"
              />
              <rect
                x={80}
                y={2}
                width={12}
                height={2.4}
                fill={SCREEN}
                stroke={rim}
                strokeWidth={1.2}
                transform="rotate(14 86 3.2)"
              />
            </>
          )}

          {(mood === "happy" || mood === "alert") && (
            <>
              <ellipse cx={20} cy={51} rx={5} ry={2.8} fill={BLUSH} opacity={0.5} />
              <ellipse cx={100} cy={51} rx={5} ry={2.8} fill={BLUSH} opacity={0.5} />
            </>
          )}

          <RimRect
            className="codex-leg codex-leg-0"
            x={38}
            y={56}
            width={9.6}
            height={20}
            fill={dark}
            stroke={rim}
          />
          <RimRect
            className="codex-leg codex-leg-1"
            x={72}
            y={56}
            width={9.6}
            height={20}
            fill={dark}
            stroke={rim}
          />
        </g>

        {mood === "alert" && (
          <g className="codex-bang">
            <rect x={53} y={-30} width={6} height={14} rx={1.5} fill="#f8dda0" />
            <rect x={53} y={-13} width={6} height={5} rx={1.5} fill="#f8dda0" />
          </g>
        )}

        {mood === "happy" && (
          <>
            <path
              className="codex-heart"
              d="M56 -14 C 51 -22 41 -17 56 -4 C 71 -17 61 -22 56 -14 Z"
              fill="#8de8ff"
            />
            <g transform="translate(-6 12)">
              <Star className="codex-star codex-star-0" />
            </g>
            <g transform="translate(118 6)">
              <Star className="codex-star codex-star-1" />
            </g>
            <g transform="translate(96 -24)">
              <Star className="codex-star codex-star-2" />
            </g>
          </>
        )}

        {mood === "sleeping" && (
          <g
            className="codex-zzz"
            fill="#aab4ff"
            fontFamily={MONO}
            fontWeight="700"
          >
            <text className="codex-z codex-z-0" x={104} y={-6} fontSize={16}>
              z
            </text>
            <text className="codex-z codex-z-1" x={116} y={-16} fontSize={20}>
              z
            </text>
            <text className="codex-z codex-z-2" x={128} y={-28} fontSize={24}>
              z
            </text>
          </g>
        )}

        {mood === "worried" && (
          <g transform="translate(108 -4)">
            <path className="codex-sweat" d="M4 0 C 8 7 0 7 4 0 Z" fill="#7cc4ff" />
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
      fill="#d4f8ff"
    />
  );
}
