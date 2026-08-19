import type { ClawdMood } from "./ClawdMascot";
import {
  DEAD_BODY,
  DEAD_BODY_TOP,
  DEAD_DARK,
  MASCOT_VIEWBOX_ATTR,
  MascotExtras,
  MascotShadow,
  SICK,
  SICK_DARK,
  mascotSizeStyle,
  mixHex,
} from "./mascotShared";

const BODY = "#4a9fd4";
const BODY_TOP = "#6eb8e6";
const DARK = "#3589b8";
const SCREEN = "#0c1018";
const SCREEN_SICK = "#102018";
const SCREEN_DEAD = "#1a1c1e";
const PROMPT = "#3de8f8";
const PROMPT_DIM = "#2a9aaa";
const BLUSH = "#b8ecff";

interface CodexPalette {
  body: string;
  bodyTop: string;
  dark: string;
  screen: string;
  prompt: string;
  promptDim: string;
  blush: string;
  sparkle: string;
  sweat: string;
}

function defaultCodexPalette(): CodexPalette {
  return {
    body: BODY,
    bodyTop: BODY_TOP,
    dark: DARK,
    screen: SCREEN,
    prompt: PROMPT,
    promptDim: PROMPT_DIM,
    blush: BLUSH,
    sparkle: "#d4f8ff",
    sweat: "#7cc4ff",
  };
}

function deriveCodexPalette(accent?: string, accentDark?: string): CodexPalette {
  if (!accent) return defaultCodexPalette();
  const dark = accentDark ?? mixHex(accent, "#000000", 0.35);
  const prompt = mixHex(accent, "#ffffff", 0.55);
  return {
    body: accent,
    bodyTop: mixHex(accent, "#ffffff", 0.18),
    dark,
    screen: SCREEN,
    prompt,
    promptDim: mixHex(accent, "#000000", 0.38),
    blush: mixHex(accent, "#ffffff", 0.45),
    sparkle: mixHex(accent, "#ffffff", 0.62),
    sweat: mixHex(accent, "#ffffff", 0.35),
  };
}

const deadPalette: CodexPalette = {
  body: DEAD_BODY,
  bodyTop: DEAD_BODY_TOP,
  dark: DEAD_DARK,
  screen: SCREEN_DEAD,
  prompt: "#b0b8bc",
  promptDim: "#888f93",
  blush: "#9aa0a4",
  sparkle: "#b0b8bc",
  sweat: "#9aa0a4",
};

function sickPalette(): CodexPalette {
  return {
    body: SICK,
    bodyTop: SICK,
    dark: SICK_DARK,
    screen: SCREEN_SICK,
    prompt: "#e8ffe8",
    promptDim: "#c8e8c8",
    blush: "#d4f5d4",
    sparkle: "#e8ffe8",
    sweat: "#a8d8a8",
  };
}

function CodexPromptFace({
  mood,
  color,
  blush,
}: {
  mood: ClawdMood;
  color: string;
  blush: string;
}) {
  const blink = mood !== "sleeping" && mood !== "dead";

  return (
    <>
      <g className="codex-prompt">
        <rect x={43} y={24} width={8} height={4} fill={color} />
        <rect x={47} y={28} width={8} height={4} fill={color} />
        <rect x={43} y={32} width={8} height={4} fill={color} />
        <rect
          className={blink ? "codex-cursor" : undefined}
          x={57}
          y={32}
          width={12}
          height={4}
          fill={color}
        />
      </g>
      {(mood === "happy" || mood === "alert") && (
        <>
          <ellipse cx={36} cy={40} rx={5} ry={2.8} fill={blush} opacity={0.65} />
          <ellipse cx={76} cy={40} rx={5} ry={2.8} fill={blush} opacity={0.65} />
        </>
      )}
    </>
  );
}

interface CodexMascotProps {
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
}: CodexMascotProps) {
  const palette =
    mood === "dead"
      ? deadPalette
      : mood === "worried"
        ? sickPalette()
        : deriveCodexPalette(accent, accentDark);
  const prompt = mood === "sleeping" ? palette.promptDim : palette.prompt;

  return (
    <span
      className={`codex is-${mood}${animated ? "" : " is-static"}${className ? ` ${className}` : ""}`}
      style={mascotSizeStyle(size)}
      aria-hidden="true"
    >
      <svg
        className="codex-svg"
        width="100%"
        height="100%"
        viewBox={MASCOT_VIEWBOX_ATTR}
        preserveAspectRatio="xMidYMid meet"
        shapeRendering="crispEdges"
      >
        <MascotShadow className="codex-shadow" />

        <g className="codex-body">
          <rect className="codex-chassis" x={24} y={8} width={64} height={48} fill={palette.body} />
          <rect x={24} y={8} width={64} height={8} fill={palette.bodyTop} />
          <rect className="codex-screen" x={32} y={18} width={48} height={28} fill={palette.screen} />
          <rect className="codex-screen-glow" x={32} y={18} width={48} height={28} fill={palette.prompt} />
          <CodexPromptFace mood={mood} color={prompt} blush={palette.blush} />

          <rect className="codex-leg codex-leg-0" x={36} y={56} width={12} height={16} fill={palette.dark} />
          <rect className="codex-leg codex-leg-1" x={64} y={56} width={12} height={16} fill={palette.dark} />

          <MascotExtras
            mood={mood}
            classPrefix="codex"
            heartFill={palette.sparkle}
            sparkle={palette.sparkle}
            sweat={palette.sweat}
          />
        </g>
      </svg>
    </span>
  );
}
