import { useEffect, useState } from "react";

export type AtollActivity =
  | "idle"
  | "coding"
  | "reading"
  | "thinking"
  | "coffee"
  | "idea"
  | "slacking"
  | "napping";

interface Palette { body: string; top: string }
const PALETTES: Record<AtollActivity, Palette> = {
  idle:     { body: "#38BDD8", top: "#5FD8EC" },
  coding:   { body: "#4A90D9", top: "#6AAAF0" },
  reading:  { body: "#D4A054", top: "#E8BC78" },
  thinking: { body: "#9B8EC8", top: "#B8ACE0" },
  coffee:   { body: "#A8785A", top: "#C49470" },
  idea:     { body: "#F0C040", top: "#F8D868" },
  slacking: { body: "#38BDD8", top: "#5FD8EC" },
  napping:  { body: "#2A8FA8", top: "#3CA8C0" },
};
const EYE = "#1A2A3A";

const VIEWBOX = { x: -16, y: -36, w: 96, h: 108 };
const ASPECT = VIEWBOX.w / VIEWBOX.h;

const IDLE_PLAY: AtollActivity[] = [
  "coding", "reading", "thinking", "coffee", "idea", "slacking", "napping",
];
const DEFAULT_IDLE_INTERVAL_SEC = 600;
const DEFAULT_IDLE_DURATION_SEC = 1200;

interface AtollLogoProps {
  activity?: AtollActivity;
  size?: number;
  className?: string;
  idleIntervalSec?: number;
  idleDurationSec?: number;
}

export function AtollLogo({
  activity = "idle",
  size = 64,
  className,
  idleIntervalSec = DEFAULT_IDLE_INTERVAL_SEC,
  idleDurationSec = DEFAULT_IDLE_DURATION_SEC,
}: AtollLogoProps) {
  const [playAct, setPlayAct] = useState<AtollActivity | null>(null);
  const [blinking, setBlinking] = useState(false);
  const [scanX, setScanX] = useState(0);

  // --- idle play cycling ---
  useEffect(() => {
    if (activity !== "idle") return;
    const intervalMs = idleIntervalSec * 1000;
    const durationMs = idleDurationSec * 1000;
    let cancelled = false;
    let timer: number;
    const next = () => {
      if (cancelled) return;
      const jitter = intervalMs * 0.3;
      timer = window.setTimeout(() => {
        if (cancelled) return;
        setPlayAct(IDLE_PLAY[Math.floor(Math.random() * IDLE_PLAY.length)]);
        const durJitter = durationMs * 0.3;
        timer = window.setTimeout(() => {
          if (cancelled) return;
          setPlayAct(null);
          next();
        }, durationMs - durJitter + Math.random() * durJitter * 2);
      }, intervalMs - jitter + Math.random() * jitter * 2);
    };
    next();
    return () => { cancelled = true; window.clearTimeout(timer); };
  }, [activity, idleIntervalSec, idleDurationSec]);

  useEffect(() => {
    if (activity !== "idle") setPlayAct(null);
  }, [activity]);

  const shown = playAct ?? activity;

  // --- blinking ---
  useEffect(() => {
    if (shown === "napping") return;
    let timer: number;
    const loop = () => {
      setBlinking(true);
      window.setTimeout(() => setBlinking(false), 150);
      timer = window.setTimeout(loop, 3000 + Math.random() * 2500);
    };
    timer = window.setTimeout(loop, 2500 + Math.random() * 2500);
    return () => window.clearTimeout(timer);
  }, [shown]);

  // --- reading eye-scan ---
  useEffect(() => {
    if (shown !== "reading") { setScanX(0); return; }
    let timer: number;
    let dir = 1;
    const tick = () => {
      setScanX(prev => {
        const next = prev + dir * 3;
        if (next > 6 || next < -6) dir *= -1;
        return prev + dir * 3;
      });
      timer = window.setTimeout(tick, 600);
    };
    timer = window.setTimeout(tick, 400);
    return () => window.clearTimeout(timer);
  }, [shown]);

  const { body, top } = PALETTES[shown];
  const eyeH = blinking ? 2 : shown === "idea" ? 16 : 12;
  const eyeY = shown === "coding" ? 28 : shown === "thinking" ? 16 : 20;

  const wrapperStyle = size
    ? { width: size * ASPECT, height: size }
    : undefined;

  return (
    <span
      className={`atoll-logo is-${shown}${className ? ` ${className}` : ""}`}
      style={wrapperStyle}
      aria-hidden="true"
    >
      <svg
        className="atoll-logo-svg"
        width="100%"
        height="100%"
        viewBox={`${VIEWBOX.x} ${VIEWBOX.y} ${VIEWBOX.w} ${VIEWBOX.h}`}
        preserveAspectRatio="xMidYMid meet"
        fill="none"
        shapeRendering="crispEdges"
      >
        {/* ===== Body ===== */}
        <rect x="4" y="12" width="56" height="40" fill={body} />
        <rect x="12" y="4" width="40" height="56" fill={body} />
        <rect x="12" y="4" width="40" height="8" fill={top} />

        {/* ===== Eyes ===== */}
        {shown === "napping" ? (
          <>
            <rect x="14" y="28" width="12" height="3" fill={EYE} />
            <rect x="38" y="28" width="12" height="3" fill={EYE} />
          </>
        ) : shown === "coffee" ? (
          <>
            <rect x="14" y="26" width="12" height="5" fill={EYE} />
            <rect x="38" y="26" width="12" height="5" fill={EYE} />
          </>
        ) : shown === "reading" ? (
          <>
            <rect x={14 + scanX} y="20" width="12" height={eyeH} fill={EYE} />
            <rect x={38 + scanX} y="20" width="12" height={eyeH} fill={EYE} />
          </>
        ) : (
          <>
            <rect x="14" y={eyeY} width="12" height={eyeH} fill={EYE} />
            <rect x="38" y={eyeY} width="12" height={eyeH} fill={EYE} />
          </>
        )}

        {/* ===== Slacking: sunglasses over eyes ===== */}
        {shown === "slacking" && (
          <g className="atoll-sunglasses">
            <rect x="10" y="18" width="44" height="4" fill="#1A1A1A" />
            <rect x="10" y="18" width="18" height="14" rx="2" fill="#1A1A1A" />
            <rect x="36" y="18" width="18" height="14" rx="2" fill="#1A1A1A" />
            <rect x="12" y="20" width="14" height="8" fill="#2A4060" />
            <rect x="38" y="20" width="14" height="8" fill="#2A4060" />
          </g>
        )}

        {/* ===== Napping: sleep cap ===== */}
        {shown === "napping" && (
          <g>
            <polygon points="20,-2 44,-2 54,-28" fill="#6B5B9A" />
            <polygon points="20,-2 44,-2 54,-28" fill="#6B5B9A" />
            <rect x="16" y="-4" width="32" height="6" fill="#8070B0" />
            <circle cx="56" cy="-30" r="5" fill="#E8E0F0" />
          </g>
        )}

        {/* ===== Coding: pixel laptop ===== */}
        {shown === "coding" && (
          <g className="atoll-laptop">
            {/* screen */}
            <rect x="6" y="42" width="52" height="28" fill="#2A2A3A" />
            <rect x="8" y="44" width="48" height="22" fill="#1A1A2A" />
            {/* code lines on screen */}
            <rect className="atoll-code-line atoll-code-0" x="10" y="47" width="20" height="2" fill="#6BE088" />
            <rect className="atoll-code-line atoll-code-1" x="10" y="51" width="30" height="2" fill="#68B8F8" />
            <rect className="atoll-code-line atoll-code-2" x="10" y="55" width="16" height="2" fill="#F8C868" />
            <rect className="atoll-code-line atoll-code-3" x="10" y="59" width="24" height="2" fill="#E080C0" />
            {/* keyboard base */}
            <rect x="2" y="70" width="60" height="6" rx="1" fill="#3A3A4A" />
            <rect x="4" y="70" width="56" height="2" fill="#4A4A5A" />
          </g>
        )}

        {/* ===== Reading: open book ===== */}
        {shown === "reading" && (
          <g className="atoll-book">
            <rect x="4" y="48" width="26" height="20" fill="#F5E6C8" />
            <rect x="34" y="48" width="26" height="20" fill="#F5E6C8" />
            <rect x="29" y="46" width="6" height="24" fill="#C4956A" />
            {/* text lines */}
            <rect x="8" y="52" width="16" height="2" fill="#C0A880" />
            <rect x="8" y="56" width="18" height="2" fill="#C0A880" />
            <rect x="8" y="60" width="12" height="2" fill="#C0A880" />
            <rect x="38" y="52" width="18" height="2" fill="#C0A880" />
            <rect x="38" y="56" width="14" height="2" fill="#C0A880" />
            <rect x="38" y="60" width="16" height="2" fill="#C0A880" />
          </g>
        )}

        {/* ===== Thinking: thought cloud + dots ===== */}
        {shown === "thinking" && (
          <g>
            <circle cx="52" cy="2" r="4" fill="white" fillOpacity="0.85" />
            <circle cx="58" cy="-8" r="3" fill="white" fillOpacity="0.85" />
            <rect x="28" y="-34" width="36" height="22" rx="8" fill="white" fillOpacity="0.9" shapeRendering="auto" />
            <circle className="atoll-dot atoll-dot-0" cx="38" cy="-23" r="3" fill="#9B8EC8" />
            <circle className="atoll-dot atoll-dot-1" cx="46" cy="-23" r="3" fill="#9B8EC8" />
            <circle className="atoll-dot atoll-dot-2" cx="54" cy="-23" r="3" fill="#9B8EC8" />
          </g>
        )}

        {/* ===== Coffee: mug + steam ===== */}
        {shown === "coffee" && (
          <g>
            {/* mug body */}
            <rect x="52" y="24" width="18" height="22" fill="#F5F0E8" />
            <rect x="54" y="26" width="14" height="16" fill="#6B4226" />
            {/* handle */}
            <rect x="70" y="28" width="6" height="4" fill="#F5F0E8" />
            <rect x="74" y="28" width="4" height="14" fill="#F5F0E8" />
            <rect x="70" y="38" width="6" height="4" fill="#F5F0E8" />
            {/* steam */}
            <rect className="atoll-steam atoll-steam-0" x="56" y="16" width="3" height="6" rx="1" fill="white" fillOpacity="0.6" shapeRendering="auto" />
            <rect className="atoll-steam atoll-steam-1" x="62" y="14" width="3" height="8" rx="1" fill="white" fillOpacity="0.5" shapeRendering="auto" />
            <rect className="atoll-steam atoll-steam-2" x="68" y="16" width="3" height="6" rx="1" fill="white" fillOpacity="0.6" shapeRendering="auto" />
          </g>
        )}

        {/* ===== Idea: lightbulb ===== */}
        {shown === "idea" && (
          <g className="atoll-bulb">
            <rect x="22" y="-30" width="20" height="24" rx="6" fill="#FFE566" shapeRendering="auto" />
            <rect x="26" y="-8" width="12" height="4" fill="#D4C45A" />
            <rect x="28" y="-4" width="8" height="3" fill="#D4C45A" />
            {/* glow rays */}
            <rect className="atoll-ray atoll-ray-0" x="14" y="-22" width="6" height="3" fill="#FFE566" />
            <rect className="atoll-ray atoll-ray-1" x="44" y="-22" width="6" height="3" fill="#FFE566" />
            <rect className="atoll-ray atoll-ray-2" x="30" y="-36" width="4" height="5" fill="#FFE566" />
          </g>
        )}

        {/* ===== Slacking: phone + fish ===== */}
        {shown === "slacking" && (
          <g>
            {/* phone in hand */}
            <rect x="-12" y="16" width="14" height="24" rx="2" fill="#2A2A3A" shapeRendering="auto" />
            <rect x="-10" y="18" width="10" height="18" fill="#4488CC" />
            {/* pixel fish swimming by */}
            <g className="atoll-fish atoll-fish-0">
              <polygon points="62,-10 72,-6 62,-2" fill="#FF9060" />
              <rect x="52" y="-10" width="12" height="8" rx="2" fill="#FFA870" shapeRendering="auto" />
              <rect x="54" y="-8" width="3" height="2" fill={EYE} />
            </g>
            <g className="atoll-fish atoll-fish-1">
              <polygon points="74,-24 84,-20 74,-16" fill="#60C8FF" />
              <rect x="64" y="-24" width="12" height="8" rx="2" fill="#78D8FF" shapeRendering="auto" />
              <rect x="66" y="-22" width="3" height="2" fill={EYE} />
            </g>
          </g>
        )}

        {/* ===== Napping: zzz ===== */}
        {shown === "napping" && (
          <g fill="#aab4ff" fontFamily="var(--font-mono, monospace)" fontWeight="700">
            <text className="atoll-z atoll-z-0" x="46" y="4" fontSize="14">z</text>
            <text className="atoll-z atoll-z-1" x="54" y="-10" fontSize="18">z</text>
            <text className="atoll-z atoll-z-2" x="62" y="-26" fontSize="22">z</text>
          </g>
        )}
      </svg>
    </span>
  );
}
