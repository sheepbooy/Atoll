import { useEffect, useState, type CSSProperties } from "react";

export type ClawdMood =
  | "sleeping"
  | "calm"
  | "alert"
  | "worried"
  | "happy"
  | "sad"
  | "dead";

export const MASCOT_VIEWBOX = { x: -20, y: -34, w: 152, h: 136 };
export const MASCOT_ASPECT = MASCOT_VIEWBOX.w / MASCOT_VIEWBOX.h;
export const MASCOT_VIEWBOX_ATTR = `${MASCOT_VIEWBOX.x} ${MASCOT_VIEWBOX.y} ${MASCOT_VIEWBOX.w} ${MASCOT_VIEWBOX.h}`;

export const MASCOT_BODY = { x: 8, y: 0, w: 96, h: 56, topH: 9 };
export const MASCOT_LEGS = [16, 30.4, 72, 86.4];
export const MASCOT_CLAW_LEFT = { x: -4, y: 25.6, w: 12, h: 14.4 };
export const MASCOT_CLAW_RIGHT = { x: 104, y: 25.6, w: 12, h: 14.4 };
export const MASCOT_SHADOW = { cx: 68, cy: 92, rx: 32, ry: 4 };

export const EYE = "#1a1a1a";
export const DEAD_EYE = "#050505";
export const SICK = "#7cb97c";
export const SICK_DARK = "#5a8b5a";
export const BLUSH = "#ffb4b4";
export const DEAD_BODY = "#8a8a8a";
export const DEAD_BODY_TOP = "#9a9a9a";
export const DEAD_DARK = "#666666";
export const HEART = "#ff8d8d";
export const SPARKLE = "#ffe6a8";
export const SWEAT = "#7cc4ff";
export const BANG = "#f8dda0";
export const ZZZ = "#aab4ff";

export function parseHex(hex: string): [number, number, number] | null {
  const match = /^#?([0-9a-f]{6})$/i.exec(hex.trim());
  if (!match) return null;
  const value = Number.parseInt(match[1], 16);
  return [(value >> 16) & 255, (value >> 8) & 255, value & 255];
}

export function rgbHex(r: number, g: number, b: number): string {
  const clamp = (channel: number) => Math.max(0, Math.min(255, Math.round(channel)));
  return `#${[clamp(r), clamp(g), clamp(b)]
    .map((channel) => channel.toString(16).padStart(2, "0"))
    .join("")}`;
}

export function mixHex(from: string, to: string, amount: number): string {
  const source = parseHex(from);
  const target = parseHex(to);
  if (!source || !target) return from;
  return rgbHex(
    source[0] + (target[0] - source[0]) * amount,
    source[1] + (target[1] - source[1]) * amount,
    source[2] + (target[2] - source[2]) * amount,
  );
}

export function mascotSizeStyle(size?: number): CSSProperties | undefined {
  if (!size) return undefined;
  return { width: size * MASCOT_ASPECT, height: size };
}

export function useMascotBlink(animated: boolean, mood: ClawdMood): boolean {
  const [blinking, setBlinking] = useState(false);

  useEffect(() => {
    if (!animated || mood === "sleeping" || mood === "dead") {
      setBlinking(false);
      return;
    }
    let loopTimer = 0;
    let blinkTimer = 0;
    const loop = () => {
      setBlinking(true);
      blinkTimer = window.setTimeout(() => setBlinking(false), 150);
      loopTimer = window.setTimeout(loop, 3000 + Math.random() * 2500);
    };
    loopTimer = window.setTimeout(loop, 2500 + Math.random() * 2500);
    return () => {
      window.clearTimeout(loopTimer);
      window.clearTimeout(blinkTimer);
    };
  }, [animated, mood]);

  return blinking;
}

export function MascotShadow({ className }: { className: string }) {
  return (
    <ellipse
      className={className}
      cx={MASCOT_SHADOW.cx}
      cy={MASCOT_SHADOW.cy}
      rx={MASCOT_SHADOW.rx}
      ry={MASCOT_SHADOW.ry}
      fill="rgba(0,0,0,0.18)"
    />
  );
}

export function MascotClaws({ classPrefix, fill }: { classPrefix: string; fill: string }) {
  return (
    <>
      <rect
        className={`${classPrefix}-claw ${classPrefix}-claw-left`}
        x={MASCOT_CLAW_LEFT.x}
        y={MASCOT_CLAW_LEFT.y}
        width={MASCOT_CLAW_LEFT.w}
        height={MASCOT_CLAW_LEFT.h}
        fill={fill}
      />
      <rect
        className={`${classPrefix}-claw ${classPrefix}-claw-right`}
        x={MASCOT_CLAW_RIGHT.x}
        y={MASCOT_CLAW_RIGHT.y}
        width={MASCOT_CLAW_RIGHT.w}
        height={MASCOT_CLAW_RIGHT.h}
        fill={fill}
      />
    </>
  );
}

export function MascotLegs({ classPrefix, fill }: { classPrefix: string; fill: string }) {
  return (
    <>
      {MASCOT_LEGS.map((x, i) => (
        <rect
          key={x}
          className={`${classPrefix}-leg ${classPrefix}-leg-${i}`}
          x={x}
          y={56}
          width={9.6}
          height={20}
          fill={fill}
        />
      ))}
    </>
  );
}

export type MascotFaceLayout = {
  leftX: number;
  rightX: number;
  eyeY: number;
  eyeW: number;
  eyeH: number;
  blushCy: number;
};

export const CLAWD_FACE: MascotFaceLayout = {
  leftX: 28,
  rightX: 76,
  eyeY: 12,
  eyeW: 8,
  eyeH: 16,
  blushCy: 42,
};

export const CURSOR_FACE: MascotFaceLayout = {
  leftX: 40,
  rightX: 64,
  eyeY: 18,
  eyeW: 8,
  eyeH: 16,
  blushCy: 46,
};

export function MascotFace({
  mood,
  blinking,
  eyeFill = EYE,
  blushFill = BLUSH,
  layout = CLAWD_FACE,
}: {
  mood: ClawdMood;
  blinking: boolean;
  eyeFill?: string;
  blushFill?: string;
  layout?: MascotFaceLayout;
}) {
  const isSick = mood === "worried";
  const { leftX, rightX, eyeY, eyeW, eyeH, blushCy } = layout;
  const eyeHeight = blinking ? 2.4 : eyeH;
  const deadPad = 2;

  return (
    <>
      {mood === "dead" ? (
        <>
          <line
            x1={leftX - deadPad}
            y1={eyeY - deadPad}
            x2={leftX + eyeW + deadPad}
            y2={eyeY + eyeH + deadPad}
            stroke={eyeFill}
            strokeWidth={3.2}
            strokeLinecap="round"
          />
          <line
            x1={leftX + eyeW + deadPad}
            y1={eyeY - deadPad}
            x2={leftX - deadPad}
            y2={eyeY + eyeH + deadPad}
            stroke={eyeFill}
            strokeWidth={3.2}
            strokeLinecap="round"
          />
          <line
            x1={rightX - deadPad}
            y1={eyeY - deadPad}
            x2={rightX + eyeW + deadPad}
            y2={eyeY + eyeH + deadPad}
            stroke={eyeFill}
            strokeWidth={3.2}
            strokeLinecap="round"
          />
          <line
            x1={rightX + eyeW + deadPad}
            y1={eyeY - deadPad}
            x2={rightX - deadPad}
            y2={eyeY + eyeH + deadPad}
            stroke={eyeFill}
            strokeWidth={3.2}
            strokeLinecap="round"
          />
        </>
      ) : isSick ? (
        <>
          <line x1={leftX} y1={eyeY} x2={leftX + eyeW} y2={eyeY + eyeH} stroke={eyeFill} strokeWidth={2.4} strokeLinecap="round" />
          <line x1={leftX + eyeW} y1={eyeY} x2={leftX} y2={eyeY + eyeH} stroke={eyeFill} strokeWidth={2.4} strokeLinecap="round" />
          <line x1={rightX} y1={eyeY} x2={rightX + eyeW} y2={eyeY + eyeH} stroke={eyeFill} strokeWidth={2.4} strokeLinecap="round" />
          <line x1={rightX + eyeW} y1={eyeY} x2={rightX} y2={eyeY + eyeH} stroke={eyeFill} strokeWidth={2.4} strokeLinecap="round" />
        </>
      ) : mood === "sleeping" ? (
        <>
          <rect x={leftX} y={eyeY + 4} width={eyeW + 1.6} height={2.4} fill={eyeFill} />
          <rect x={rightX} y={eyeY + 4} width={eyeW + 1.6} height={2.4} fill={eyeFill} />
        </>
      ) : (
        <>
          <rect x={leftX} y={eyeY} width={eyeW} height={eyeHeight} fill={eyeFill} />
          <rect x={rightX} y={eyeY} width={eyeW} height={eyeHeight} fill={eyeFill} />
        </>
      )}

      {mood === "sad" && (
        <>
          <rect
            x={leftX - 4}
            y={eyeY - 4}
            width={12}
            height={2.4}
            fill={eyeFill}
            transform={`rotate(-15 ${leftX + 2} ${eyeY - 2.8})`}
          />
          <rect
            x={rightX}
            y={eyeY - 4}
            width={12}
            height={2.4}
            fill={eyeFill}
            transform={`rotate(15 ${rightX + 6} ${eyeY - 2.8})`}
          />
        </>
      )}

      {(mood === "happy" || mood === "alert") && (
        <>
          <ellipse cx={leftX - 6} cy={blushCy} rx={6} ry={3.2} fill={blushFill} opacity={0.65} />
          <ellipse cx={rightX + eyeW + 6} cy={blushCy} rx={6} ry={3.2} fill={blushFill} opacity={0.65} />
        </>
      )}
    </>
  );
}

function Star({ className, fill }: { className: string; fill: string }) {
  return (
    <polygon
      className={className}
      points="0,-6 1.6,-1.6 6,0 1.6,1.6 0,6 -1.6,1.6 -6,0 -1.6,-1.6"
      fill={fill}
    />
  );
}

export function MascotExtras({
  mood,
  classPrefix,
  heartFill = HEART,
  sparkle = SPARKLE,
  sweat = SWEAT,
}: {
  mood: ClawdMood;
  classPrefix: string;
  heartFill?: string;
  sparkle?: string;
  sweat?: string;
}) {
  if (mood === "alert") {
    return (
      <g className={`${classPrefix}-bang`}>
        <rect x={53} y={-30} width={6} height={14} rx={1.5} fill={BANG} />
        <rect x={53} y={-13} width={6} height={5} rx={1.5} fill={BANG} />
      </g>
    );
  }

  if (mood === "happy") {
    return (
      <>
        <path
          className={`${classPrefix}-heart`}
          d="M56 -14 C 51 -22 41 -17 56 -4 C 71 -17 61 -22 56 -14 Z"
          fill={heartFill}
        />
        <g transform="translate(-6 12)">
          <Star className={`${classPrefix}-star ${classPrefix}-star-0`} fill={sparkle} />
        </g>
        <g transform="translate(118 6)">
          <Star className={`${classPrefix}-star ${classPrefix}-star-1`} fill={sparkle} />
        </g>
        <g transform="translate(96 -24)">
          <Star className={`${classPrefix}-star ${classPrefix}-star-2`} fill={sparkle} />
        </g>
      </>
    );
  }

  if (mood === "sleeping") {
    return (
      <g
        className={`${classPrefix}-zzz`}
        fill={ZZZ}
        fontFamily="var(--font-mono, monospace)"
        fontWeight="700"
      >
        <text className={`${classPrefix}-z ${classPrefix}-z-0`} x={104} y={-6} fontSize={16}>
          z
        </text>
        <text className={`${classPrefix}-z ${classPrefix}-z-1`} x={116} y={-16} fontSize={20}>
          z
        </text>
        <text className={`${classPrefix}-z ${classPrefix}-z-2`} x={128} y={-28} fontSize={24}>
          z
        </text>
      </g>
    );
  }

  if (mood === "worried") {
    return (
      <g transform="translate(108 -4)">
        <path className={`${classPrefix}-sweat`} d="M4 0 C 8 7 0 7 4 0 Z" fill={sweat} />
      </g>
    );
  }

  return null;
}
