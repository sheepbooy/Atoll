import { useEffect, useState, type CSSProperties, type ReactNode } from "react";
import type { ClawdMood } from "./ClawdMascot";

const EYE = "#1a1a1a";
const EYE_W = 5;
const EYE_H = 11;
const EYE_H_WIDE = 12;
const EYE_BLINK_H = 2;
const EYE_BLINK_W = 8;
const SICK = "#7cb97c";
const SICK_DARK = "#4a8a4a";
const BLUSH = "#f0e8ff";

interface CursorCubePalette {
  top: string;
  left: string;
  front: string;
  facet: string;
  groove: string;
  outline: string;
  blush: string;
  sparkle: string;
  sweat: string;
  glowInner: string;
  glowOuter: string;
}

function defaultCubePalette(): CursorCubePalette {
  return {
    top: "#ede9fe",
    left: "#c4b5fd",
    front: "#8b6fd8",
    facet: "#ddd6fe",
    groove: "#6d4fc9",
    outline: "#b8a4f8",
    blush: BLUSH,
    sparkle: "#f5f3ff",
    sweat: "#c4b5fd",
    glowInner: "rgba(196, 181, 253, 0.75)",
    glowOuter: "rgba(124, 95, 212, 0.28)",
  };
}

const deadCubePalette: CursorCubePalette = {
  top: "#a4a8ac",
  left: "#909498",
  front: "#787c80",
  facet: "#b0b4b8",
  groove: "#585c60",
  outline: "#9aa0a4",
  blush: "#b0b4b8",
  sparkle: "#c8ccd0",
  sweat: "#a0a4a8",
  glowInner: "rgba(140, 148, 152, 0.45)",
  glowOuter: "rgba(80, 88, 92, 0.28)",
};

function parseHex(hex: string): [number, number, number] | null {
  const normalized = hex.replace("#", "").trim();
  if (normalized.length !== 6) return null;
  const value = Number.parseInt(normalized, 16);
  if (Number.isNaN(value)) return null;
  return [(value >> 16) & 255, (value >> 8) & 255, value & 255];
}

function mixHex(a: string, b: string, weight: number): string {
  const left = parseHex(a);
  const right = parseHex(b);
  if (!left || !right) return a;
  const mix = (l: number, r: number) => Math.round(l * (1 - weight) + r * weight);
  const rgb = [mix(left[0], right[0]), mix(left[1], right[1]), mix(left[2], right[2])];
  return `#${rgb.map((channel) => channel.toString(16).padStart(2, "0")).join("")}`;
}

function deriveCubePalette(accent?: string, accentDark?: string): CursorCubePalette {
  if (!accent) return defaultCubePalette();

  const dark = accentDark ?? mixHex(accent, "#000000", 0.35);
  const glowRgb = parseHex(accent);
  const glowOuter = glowRgb
    ? `rgba(${glowRgb[0]}, ${glowRgb[1]}, ${glowRgb[2]}, 0.28)`
    : "rgba(124, 95, 212, 0.28)";
  const glowInner = glowRgb
    ? `rgba(${Math.min(glowRgb[0] + 40, 255)}, ${Math.min(glowRgb[1] + 40, 255)}, ${Math.min(glowRgb[2] + 40, 255)}, 0.75)`
    : "rgba(196, 181, 253, 0.75)";

  return {
    top: mixHex(accent, "#ffffff", 0.42),
    left: mixHex(accent, "#ffffff", 0.18),
    front: dark,
    facet: mixHex(accent, "#ffffff", 0.55),
    groove: mixHex(dark, "#000000", 0.28),
    outline: mixHex(accent, "#ffffff", 0.35),
    blush: mixHex(accent, "#ffffff", 0.45),
    sparkle: mixHex(accent, "#ffffff", 0.62),
    sweat: mixHex(accent, "#ffffff", 0.35),
    glowInner,
    glowOuter,
  };
}

const VIEWBOX = { x: -20, y: -34, w: 152, h: 136 };
const ASPECT = VIEWBOX.w / VIEWBOX.h;
const MONO = "ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace";
/** Scale cube to match Clawd/Codex body footprint in the shared viewBox. */
const CUBE_SCALE = 1.58;
const CUBE_ANCHOR = { x: 60, y: 38 };
const cubeTransform = `translate(${CUBE_ANCHOR.x} ${CUBE_ANCHOR.y}) scale(${CUBE_SCALE}) translate(${-CUBE_ANCHOR.x} ${-CUBE_ANCHOR.y})`;

/** Isometric cube faces — matches Cursor brand cube silhouette. */
const CUBE = {
  top: "32,18 60,4 88,18 60,32",
  /** Left face — primary expression surface (most visible in isometric view). */
  left: "32,18 60,32 60,68 32,54",
  /** Front-right logo facet (no eyes here). */
  front: "60,32 88,18 88,54 60,68",
  facet: "88,18 74,28 88,42",
  groove: { x1: 74, y1: 28, x2: 64, y2: 60 },
  /** Left-face UV: origin at top-right, u → top-left, v → down. */
  leftOrigin: { x: 60, y: 32 },
  leftU: { x: -28, y: -14 },
  leftV: { x: 0, y: 36 },
};

interface EyeSlot {
  cx: number;
  cy: number;
}

/** Eyes float on the cube body — centered horizontally, slightly above vertical midpoint. */
const FLOATING_EYES: [EyeSlot, EyeSlot] = [
  { cx: 47, cy: 33 },
  { cx: 73, cy: 33 },
];

const FLOATING_MOUTH: EyeSlot = { cx: 60, cy: 39 };

function eyePair(): [EyeSlot, EyeSlot] {
  return FLOATING_EYES;
}

/** Soft pad so eyes read as stickers floating on the cube. */
function eyeFloatPad(cx: number, cy: number) {
  return (
    <ellipse
      cx={cx}
      cy={cy + 0.5}
      rx={4.8}
      ry={6.8}
      fill="rgba(255,255,255,0.2)"
    />
  );
}

/** Atoll-style vertical eye rect, centered on slot. */
function verticalEye(cx: number, cy: number, width = EYE_W, height = EYE_H, fill = EYE) {
  return (
    <rect
      x={cx - width / 2}
      y={cy - height / 2}
      width={width}
      height={height}
      fill={fill}
    />
  );
}

function blinkEye(cx: number, cy: number) {
  return (
    <rect
      x={cx - EYE_BLINK_W / 2}
      y={cy - EYE_BLINK_H / 2}
      width={EYE_BLINK_W}
      height={EYE_BLINK_H}
      fill={EYE}
    />
  );
}

function sleepEye(cx: number, cy: number) {
  return blinkEye(cx, cy);
}

/** Atoll happy — flat horizontal eye bars. */
function happyEye(cx: number, cy: number) {
  return (
    <rect
      x={cx - EYE_BLINK_W / 2}
      y={cy - EYE_BLINK_H / 2}
      width={EYE_BLINK_W}
      height={EYE_BLINK_H}
      fill={EYE}
    />
  );
}

function deadEye(cx: number, cy: number) {
  const stroke = "#e8eaed";
  return (
    <>
      <line
        x1={cx - 4.5}
        y1={cy - 5.5}
        x2={cx + 4.5}
        y2={cy + 5.5}
        stroke={stroke}
        strokeWidth={2.8}
        strokeLinecap="round"
      />
      <line
        x1={cx + 4.5}
        y1={cy - 5.5}
        x2={cx - 4.5}
        y2={cy + 5.5}
        stroke={stroke}
        strokeWidth={2.8}
        strokeLinecap="round"
      />
    </>
  );
}

function brow(
  cx: number,
  cy: number,
  tilt: "sad" | "worried",
  index: 0 | 1,
) {
  const y = cy - EYE_H / 2 - 5;
  const w = 7;
  const angle = tilt === "sad" ? (index === 0 ? -18 : 18) : index === 0 ? 18 : -18;
  return (
    <rect
      x={cx - w / 2}
      y={y}
      width={w}
      height={2}
      fill={EYE}
      rx={0.4}
      transform={`rotate(${angle} ${cx} ${y})`}
    />
  );
}

interface CursorMascotProps {
  mood: ClawdMood;
  size?: number;
  className?: string;
  accent?: string;
  accentDark?: string;
}

function CubeFace({
  palette,
  strokeWidth = 2,
}: {
  palette: CursorCubePalette;
  strokeWidth?: number;
}) {
  return (
    <>
      <polygon
        className="cursor-mascot-face cursor-mascot-face-left"
        points={CUBE.left}
        fill={palette.left}
        stroke={palette.outline}
        strokeWidth={strokeWidth}
        strokeLinejoin="round"
      />
      <polygon
        className="cursor-mascot-face cursor-mascot-face-front"
        points={CUBE.front}
        fill={palette.front}
        stroke={palette.outline}
        strokeWidth={strokeWidth}
        strokeLinejoin="round"
      />
      <polygon
        className="cursor-mascot-face cursor-mascot-face-top"
        points={CUBE.top}
        fill={palette.top}
        stroke={palette.outline}
        strokeWidth={strokeWidth}
        strokeLinejoin="round"
      />
      <polygon
        className="cursor-mascot-facet"
        points={CUBE.facet}
        fill={palette.facet}
        stroke={palette.outline}
        strokeWidth={1.5}
        strokeLinejoin="round"
      />
      <line
        className="cursor-mascot-groove"
        x1={CUBE.groove.x1}
        y1={CUBE.groove.y1}
        x2={CUBE.groove.x2}
        y2={CUBE.groove.y2}
        stroke={palette.groove}
        strokeWidth={2.4}
        strokeLinecap="round"
      />
    </>
  );
}

function CubeExpression({
  mood,
  blinking,
}: {
  mood: ClawdMood;
  blinking: boolean;
}) {
  const [left, right] = eyePair();

  function floatingEye(eye: EyeSlot, content: ReactNode, key: number) {
    return (
      <g key={key} className="cursor-mascot-eye-float">
        {eyeFloatPad(eye.cx, eye.cy)}
        {content}
      </g>
    );
  }

  if (mood === "dead") {
    return (
      <g className="cursor-mascot-eyes cursor-mascot-eyes-dead">
        {[left, right].map((eye, index) =>
          floatingEye(eye, deadEye(eye.cx, eye.cy), index),
        )}
      </g>
    );
  }

  if (mood === "happy") {
    const mouth = FLOATING_MOUTH;
    return (
      <g className="cursor-mascot-eyes">
        {[left, right].map((eye, index) =>
          floatingEye(eye, happyEye(eye.cx, eye.cy), index),
        )}
        <rect
          x={mouth.cx - 4}
          y={mouth.cy}
          width={8}
          height={2}
          fill={EYE}
          rx={0.5}
          opacity={0.85}
        />
      </g>
    );
  }

  if (mood === "sleeping") {
    return (
      <g className="cursor-mascot-eyes">
        {[left, right].map((eye, index) =>
          floatingEye(eye, sleepEye(eye.cx, eye.cy), index),
        )}
      </g>
    );
  }

  if (mood === "worried") {
    return (
      <g className="cursor-mascot-eyes">
        {[left, right].map((eye, index) =>
          floatingEye(
            eye,
            <>
              {verticalEye(eye.cx, eye.cy)}
              {brow(eye.cx, eye.cy, "worried", index as 0 | 1)}
            </>,
            index,
          ),
        )}
      </g>
    );
  }

  if (mood === "sad") {
    return (
      <g className="cursor-mascot-eyes">
        {[left, right].map((eye, index) =>
          floatingEye(
            eye,
            <>
              {verticalEye(eye.cx, eye.cy)}
              {brow(eye.cx, eye.cy, "sad", index as 0 | 1)}
            </>,
            index,
          ),
        )}
      </g>
    );
  }

  return (
    <g className="cursor-mascot-eyes">
      {[left, right].map((eye, index) =>
        floatingEye(
          eye,
          blinking
            ? blinkEye(eye.cx, eye.cy)
            : verticalEye(
                eye.cx,
                eye.cy,
                EYE_W,
                mood === "alert" ? EYE_H_WIDE : EYE_H,
              ),
          index,
        ),
      )}
    </g>
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

export function CursorMascot({
  mood,
  size,
  className,
  accent,
  accentDark,
}: CursorMascotProps) {
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
  const palette = isDead
    ? deadCubePalette
    : isSick
      ? {
          ...defaultCubePalette(),
          top: SICK,
          left: mixHex(SICK, "#ffffff", 0.15),
          front: SICK_DARK,
          facet: mixHex(SICK, "#ffffff", 0.35),
          groove: mixHex(SICK_DARK, "#000000", 0.2),
          outline: mixHex(SICK, "#ffffff", 0.25),
          blush: "#d4f5d4",
          sparkle: "#e8ffe8",
          sweat: "#a8d8a8",
          glowInner: "rgba(212, 245, 212, 0.75)",
          glowOuter: "rgba(124, 185, 124, 0.28)",
        }
      : deriveCubePalette(accent, accentDark);

  const wrapperStyle = {
    ...(size ? { width: size * ASPECT, height: size } : {}),
    "--cursor-mascot-glow-inner": palette.glowInner,
    "--cursor-mascot-glow-outer": palette.glowOuter,
  } as CSSProperties;

  return (
    <span
      className={`cursor-mascot is-${mood}${className ? ` ${className}` : ""}`}
      style={wrapperStyle}
      aria-hidden="true"
    >
      <svg
        className="cursor-mascot-svg"
        width="100%"
        height="100%"
        viewBox={`${VIEWBOX.x} ${VIEWBOX.y} ${VIEWBOX.w} ${VIEWBOX.h}`}
        preserveAspectRatio="xMidYMid meet"
      >
        <g className="cursor-mascot-figure" transform={cubeTransform}>
          <ellipse
            className="cursor-mascot-shadow"
            cx={60}
            cy={78}
            rx={30}
            ry={4.5}
            fill="rgba(0,0,0,0.22)"
          />

          <g className="cursor-mascot-cube" shapeRendering="crispEdges">
            <CubeFace palette={palette} />
          </g>

          <CubeExpression mood={mood} blinking={blinking} />

          {mood === "alert" && (
            <g className="cursor-mascot-bang">
              <rect x={53} y={-30} width={6} height={14} rx={1.5} fill="#f8dda0" />
              <rect x={53} y={-13} width={6} height={5} rx={1.5} fill="#f8dda0" />
            </g>
          )}

          {mood === "happy" && (
            <>
              <path
                className="cursor-mascot-heart"
                d="M56 -14 C 51 -22 41 -17 56 -4 C 71 -17 61 -22 56 -14 Z"
                fill={palette.sparkle}
              />
              <g transform="translate(-6 12)">
                <Star className="cursor-mascot-star cursor-mascot-star-0" fill={palette.sparkle} />
              </g>
              <g transform="translate(118 6)">
                <Star className="cursor-mascot-star cursor-mascot-star-1" fill={palette.sparkle} />
              </g>
              <g transform="translate(96 -24)">
                <Star className="cursor-mascot-star cursor-mascot-star-2" fill={palette.sparkle} />
              </g>
            </>
          )}

          {mood === "sleeping" && (
            <g className="cursor-mascot-zzz" fill="#aab4ff" fontFamily={MONO} fontWeight="700">
              <text className="cursor-mascot-z cursor-mascot-z-0" x={104} y={-6} fontSize={16}>
                z
              </text>
              <text className="cursor-mascot-z cursor-mascot-z-1" x={116} y={-16} fontSize={20}>
                z
              </text>
              <text className="cursor-mascot-z cursor-mascot-z-2" x={128} y={-28} fontSize={24}>
                z
              </text>
            </g>
          )}

          {mood === "worried" && (
            <g transform="translate(108 -4)">
              <path
                className="cursor-mascot-sweat"
                d="M4 0 C 8 7 0 7 4 0 Z"
                fill={palette.sweat}
              />
            </g>
          )}
        </g>
      </svg>
    </span>
  );
}
