import type { CSSProperties } from "react";
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

interface CodexPalette {
  body: string;
  bodyTop: string;
  dark: string;
  bezel: string;
  bezelRim: string;
  outline: string;
  prompt: string;
  promptDim: string;
  blush: string;
  sparkle: string;
  sweat: string;
  glowInner: string;
  glowOuter: string;
}

function parseHex(hex: string): [number, number, number] | null {
  const match = /^#?([0-9a-f]{6})$/i.exec(hex.trim());
  if (!match) return null;
  const value = Number.parseInt(match[1], 16);
  return [(value >> 16) & 255, (value >> 8) & 255, value & 255];
}

function rgbHex(r: number, g: number, b: number): string {
  const clamp = (channel: number) =>
    Math.max(0, Math.min(255, Math.round(channel)));
  return `#${[clamp(r), clamp(g), clamp(b)]
    .map((channel) => channel.toString(16).padStart(2, "0"))
    .join("")}`;
}

function mixHex(from: string, to: string, amount: number): string {
  const source = parseHex(from);
  const target = parseHex(to);
  if (!source || !target) return from;
  return rgbHex(
    source[0] + (target[0] - source[0]) * amount,
    source[1] + (target[1] - source[1]) * amount,
    source[2] + (target[2] - source[2]) * amount,
  );
}

function defaultCodexPalette(): CodexPalette {
  return {
    body: BODY,
    bodyTop: BODY_TOP,
    dark: DARK,
    bezel: BEZEL,
    bezelRim: BEZEL_RIM,
    outline: OUTLINE,
    prompt: PROMPT,
    promptDim: PROMPT_DIM,
    blush: BLUSH,
    sparkle: "#d4f8ff",
    sweat: "#7cc4ff",
    glowInner: "rgba(158, 220, 255, 0.75)",
    glowOuter: "rgba(56, 168, 220, 0.28)",
  };
}

function deriveCodexPalette(accent?: string, accentDark?: string): CodexPalette {
  if (!accent) return defaultCodexPalette();

  const dark = accentDark ?? mixHex(accent, "#000000", 0.35);
  const glowRgb = parseHex(accent);
  const glowOuter = glowRgb
    ? `rgba(${glowRgb[0]}, ${glowRgb[1]}, ${glowRgb[2]}, 0.28)`
    : "rgba(56, 168, 220, 0.28)";
  const glowInner = glowRgb
    ? `rgba(${Math.min(glowRgb[0] + 40, 255)}, ${Math.min(glowRgb[1] + 40, 255)}, ${Math.min(glowRgb[2] + 40, 255)}, 0.75)`
    : "rgba(158, 220, 255, 0.75)";

  return {
    body: accent,
    bodyTop: mixHex(accent, "#ffffff", 0.18),
    dark,
    bezel: mixHex(dark, "#000000", 0.22),
    bezelRim: mixHex(accent, "#ffffff", 0.12),
    outline: mixHex(accent, "#ffffff", 0.42),
    prompt: mixHex(accent, "#ffffff", 0.55),
    promptDim: mixHex(accent, "#000000", 0.38),
    blush: mixHex(accent, "#ffffff", 0.45),
    sparkle: mixHex(accent, "#ffffff", 0.62),
    sweat: mixHex(accent, "#ffffff", 0.35),
    glowInner,
    glowOuter,
  };
}

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

  if (mood === "dead") {
    return (
      <text
        {...base}
        fill={color}
        opacity={0.95}
        fontSize={PROMPT_SIZE + 10}
        letterSpacing={-1}
      >
        xx
      </text>
    );
  }

  if (mood === "worried") {
    return (
      <text {...base} fill={color} opacity={1}>
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
  const isDead = mood === "dead";
  const isSick = mood === "worried";
  const deadPalette: CodexPalette = {
    body: "#7a8488",
    bodyTop: "#929a9e",
    dark: "#565c60",
    bezel: "#4a5256",
    bezelRim: "#6a7276",
    outline: "#8a9296",
    prompt: "#b0b8bc",
    promptDim: "#888f93",
    blush: "#9aa0a4",
    sparkle: "#b0b8bc",
    sweat: "#9aa0a4",
    glowInner: "rgba(140, 148, 152, 0.45)",
    glowOuter: "rgba(80, 88, 92, 0.28)",
  };
  const palette = isDead
    ? deadPalette
    : isSick
    ? {
        ...defaultCodexPalette(),
        body: SICK,
        bodyTop: SICK,
        dark: SICK_DARK,
        outline: OUTLINE_SICK,
        bezel: SICK_DARK,
        bezelRim: OUTLINE_SICK,
        prompt: "#e8ffe8",
        promptDim: "#c8e8c8",
        blush: "#d4f5d4",
        sparkle: "#e8ffe8",
        sweat: "#a8d8a8",
        glowInner: "rgba(212, 245, 212, 0.75)",
        glowOuter: "rgba(124, 185, 124, 0.28)",
      }
    : deriveCodexPalette(accent, accentDark);
  const prompt =
    mood === "sleeping" ? palette.promptDim : palette.prompt;

  const wrapperStyle = {
    ...(size ? { width: size * ASPECT, height: size } : {}),
    "--codex-glow-inner": palette.glowInner,
    "--codex-glow-outer": palette.glowOuter,
  } as CSSProperties;

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
          <RimRect
            x={38}
            y={-5}
            width={32}
            height={6}
            fill={palette.body}
            stroke={palette.outline}
          />

          <RimRect
            x={16}
            y={0}
            width={88}
            height={56}
            fill={palette.body}
            stroke={palette.outline}
          />
          <rect x={16} y={0} width={88} height={6} fill={palette.bodyTop} />

          <RimRect
            className="codex-screen"
            x={18}
            y={5}
            width={84}
            height={49}
            fill={palette.bezel}
            stroke={palette.bezelRim}
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
                stroke={palette.outline}
                strokeWidth={1.2}
                transform="rotate(-14 26 3.2)"
              />
              <rect
                x={80}
                y={2}
                width={12}
                height={2.4}
                fill={SCREEN}
                stroke={palette.outline}
                strokeWidth={1.2}
                transform="rotate(14 86 3.2)"
              />
            </>
          )}

          {(mood === "happy" || mood === "alert") && (
            <>
              <ellipse cx={20} cy={51} rx={5} ry={2.8} fill={palette.blush} opacity={0.5} />
              <ellipse cx={100} cy={51} rx={5} ry={2.8} fill={palette.blush} opacity={0.5} />
            </>
          )}

          <RimRect
            className="codex-leg codex-leg-0"
            x={38}
            y={56}
            width={9.6}
            height={20}
            fill={palette.dark}
            stroke={palette.outline}
          />
          <RimRect
            className="codex-leg codex-leg-1"
            x={72}
            y={56}
            width={9.6}
            height={20}
            fill={palette.dark}
            stroke={palette.outline}
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
              fill={palette.sparkle}
            />
            <g transform="translate(-6 12)">
              <Star className="codex-star codex-star-0" fill={palette.sparkle} />
            </g>
            <g transform="translate(118 6)">
              <Star className="codex-star codex-star-1" fill={palette.sparkle} />
            </g>
            <g transform="translate(96 -24)">
              <Star className="codex-star codex-star-2" fill={palette.sparkle} />
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
            <path
              className="codex-sweat"
              d="M4 0 C 8 7 0 7 4 0 Z"
              fill={palette.sweat}
            />
          </g>
        )}
      </svg>
    </span>
  );
}

function Star({ className, fill }: { className?: string; fill: string }) {
  return (
    <polygon
      className={className}
      points="0,-6 1.6,-1.6 6,0 1.6,1.6 0,6 -1.6,1.6 -6,0 -1.6,-1.6"
      fill={fill}
    />
  );
}
