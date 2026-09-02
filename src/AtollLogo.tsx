import { useEffect, useRef, useState } from "react";
import { ATOLL_ENTER_MS, ATOLL_EXIT_MS, ATOLL_REACTION_MS } from "./atollTransitions";
import { IDLE_EASTER_EGG_ACTIVITIES } from "./logoStates";
import { useAtollPhase } from "./useAtollPhase";

export type AtollActivity =
  | "idle"
  | "coding"
  | "reading"
  | "thinking"
  | "coffee"
  | "idea"
  | "slacking"
  | "napping"
  | "dead"
  | "fishing"
  | "stargazing"
  | "garden"
  | "music"
  | "gaming";

/** 状态切换时的一次性过场（与姿态 enter/exit 并行播放）。 */
export type AtollReaction = "cheer" | "collapse" | "revive";

/** idle 时随机插入的一次性微动作（叠加在浮空之上）。 */
export type AtollMicroMotion = "yawn" | "stretch" | "look" | "hop";

export const ATOLL_MICRO_MOTIONS: AtollMicroMotion[] = ["yawn", "stretch", "look", "hop"];

/** 微动作持续时长（与 CSS is-micro-* 动画长度保持一致）。 */
export const ATOLL_MICRO_MS = 1800;

const IDLE_PALETTE = { body: "#38BDD8", top: "#5FD8EC" };
const DEAD_PALETTE = { body: "#6E8A94", top: "#8AA4AE" };

const EYE = "#1a1a1a";
const DEAD_EYE = "#050505";
const BX = 8;
const BY = 18;
const BW = 48;
const BH = 26;
const BTOP = 5;
const EYE_Y = 24;

const VIEWBOX = { x: -16, y: -36, w: 96, h: 108 };
const ASPECT = VIEWBOX.w / VIEWBOX.h;

const DEFAULT_IDLE_INTERVAL_SEC = 600;
const DEFAULT_IDLE_DURATION_SEC = 1200;

interface AtollLogoProps {
  activity?: AtollActivity;
  size?: number;
  className?: string;
  idleIntervalSec?: number;
  idleDurationSec?: number;
  /** Pause blink / easter-egg / micro timers during island resize animations. */
  motionPaused?: boolean;
  /** 一次性状态反应；重复触发同一反应时递增 reactionKey 以重放。 */
  reaction?: AtollReaction | null;
  reactionKey?: number;
  /** 外部强制微动作（调试台 / 测试用）；提供时停用内部调度。 */
  microMotion?: AtollMicroMotion | null;
}

type EyeVariant = "normal" | "closed" | "happy" | "wide" | "dead";

/** 每个姿态的基础表情 + 循环内切换的备用眼（CSS 窗口互斥显隐）。 */
const ACTIVITY_EYES: Record<AtollActivity, { base: EyeVariant; alt: EyeVariant[] }> = {
  idle: { base: "normal", alt: ["wide", "happy"] },
  coding: { base: "normal", alt: ["wide"] },
  reading: { base: "normal", alt: [] },
  thinking: { base: "normal", alt: ["wide", "happy"] },
  coffee: { base: "happy", alt: ["normal"] },
  idea: { base: "wide", alt: ["happy"] },
  slacking: { base: "normal", alt: ["happy"] },
  napping: { base: "closed", alt: [] },
  dead: { base: "dead", alt: ["wide"] },
  fishing: { base: "normal", alt: ["wide", "happy"] },
  stargazing: { base: "normal", alt: ["wide", "happy"] },
  garden: { base: "normal", alt: ["happy"] },
  music: { base: "happy", alt: ["wide"] },
  gaming: { base: "normal", alt: ["wide", "happy", "closed"] },
};

function prefersReducedMotion(): boolean {
  if (typeof window === "undefined" || typeof window.matchMedia !== "function") return false;
  return window.matchMedia("(prefers-reduced-motion: reduce)").matches;
}

function EyeSet({ variant, blinking, offsetX }: { variant: EyeVariant; blinking: boolean; offsetX: number }) {
  const closed = variant === "closed" || blinking;
  const eyeH = closed ? 2.5 : variant === "wide" ? 12 : 11;
  const eyeY = closed ? EYE_Y + 4 : variant === "happy" ? EYE_Y + 5 : EYE_Y;

  if (variant === "dead") {
    return (
      <>
        <line x1={16.5 + offsetX} y1={eyeY - 1} x2={25.5 + offsetX} y2={eyeY + 12} stroke={DEAD_EYE} strokeWidth={3} strokeLinecap="round" />
        <line x1={25.5 + offsetX} y1={eyeY - 1} x2={16.5 + offsetX} y2={eyeY + 12} stroke={DEAD_EYE} strokeWidth={3} strokeLinecap="round" />
        <line x1={36.5 + offsetX} y1={eyeY - 1} x2={45.5 + offsetX} y2={eyeY + 12} stroke={DEAD_EYE} strokeWidth={3} strokeLinecap="round" />
        <line x1={45.5 + offsetX} y1={eyeY - 1} x2={36.5 + offsetX} y2={eyeY + 12} stroke={DEAD_EYE} strokeWidth={3} strokeLinecap="round" />
      </>
    );
  }
  if (variant === "happy" && !blinking) {
    return (
      <>
        <rect x={17 + offsetX} y={eyeY} width={9} height={2.5} fill={EYE} />
        <rect x={36 + offsetX} y={eyeY} width={9} height={2.5} fill={EYE} />
      </>
    );
  }
  if (closed) {
    return (
      <>
        <rect x={18 + offsetX} y={eyeY} width={6} height={2.5} fill={EYE} />
        <rect x={38 + offsetX} y={eyeY} width={6} height={2.5} fill={EYE} />
      </>
    );
  }
  return (
    <>
      <rect x={19 + offsetX} y={eyeY} width={5} height={eyeH} fill={EYE} />
      <rect x={38 + offsetX} y={eyeY} width={5} height={eyeH} fill={EYE} />
    </>
  );
}

interface MascotBodyProps {
  baseVariant: EyeVariant;
  altVariants: EyeVariant[];
  blinking: boolean;
  eyeOffsetX: number;
}

function MascotBody({ baseVariant, altVariants, blinking, eyeOffsetX }: MascotBodyProps) {
  const blinkable = baseVariant === "normal" || baseVariant === "wide";
  return (
    <g className="atoll-body-group">
      <g className="pal-color">
        <rect x={BX} y={BY} width={BW} height={BH} fill={IDLE_PALETTE.body} />
        <rect x={BX} y={BY} width={BW} height={BTOP} fill={IDLE_PALETTE.top} />
      </g>
      <g className="pal-gray">
        <rect x={BX} y={BY} width={BW} height={BH} fill={DEAD_PALETTE.body} />
        <rect x={BX} y={BY} width={BW} height={BTOP} fill={DEAD_PALETTE.top} />
      </g>
      <g className="atoll-blush">
        <ellipse cx="17" cy="34" rx="4" ry="2" fill="#ffb4b4" shapeRendering="auto" />
        <ellipse cx="47" cy="34" rx="4" ry="2" fill="#ffb4b4" shapeRendering="auto" />
      </g>
      <g className="atoll-eyes atoll-eyes-base">
        <EyeSet variant={baseVariant} blinking={blinking && blinkable} offsetX={eyeOffsetX} />
      </g>
      {altVariants.map((variant) => (
        <g key={variant} className={`atoll-eyes atoll-eyes-alt atoll-eyes-${variant}`}>
          <EyeSet variant={variant} blinking={false} offsetX={eyeOffsetX} />
        </g>
      ))}
      <g className="atoll-mouth">
        <ellipse cx="32" cy="35.5" rx="5" ry="4.5" fill="#15323c" shapeRendering="auto" />
        <ellipse cx="32" cy="37" rx="3" ry="2" fill="#c46a6a" shapeRendering="auto" />
      </g>
    </g>
  );
}

function SkyStar({ x, y, scale, cls }: { x: number; y: number; scale: number; cls: string }) {
  return (
    <g transform={`translate(${x},${y}) scale(${scale})`}>
      <path className={`atoll-star ${cls}`} d="M0 -3 L0.9 -0.9 L3 0 L0.9 0.9 L0 3 L-0.9 0.9 L-3 0 L-0.9 -0.9 Z" fill="#F4E9C8" />
    </g>
  );
}

const SPARK_D = "M0 0 l1.2 2.6 2.6 1.2 -2.6 1.2 -1.2 2.6 -1.2 -2.6 -2.6 -1.2 2.6 -1.2 Z";

export function AtollLogo({
  activity = "idle",
  size = 64,
  className,
  idleIntervalSec = DEFAULT_IDLE_INTERVAL_SEC,
  idleDurationSec = DEFAULT_IDLE_DURATION_SEC,
  motionPaused = false,
  reaction = null,
  reactionKey = 0,
  microMotion = null,
}: AtollLogoProps) {
  const [playAct, setPlayAct] = useState<AtollActivity | null>(null);
  const [blinking, setBlinking] = useState(false);
  const [scanX, setScanX] = useState(0);
  const [micro, setMicro] = useState<AtollMicroMotion | null>(null);
  const [reactionActive, setReactionActive] = useState<AtollReaction | null>(null);

  const targetAct = activity === "idle" ? (playAct ?? "idle") : activity;
  const { renderAct, phase } = useAtollPhase(targetAct);

  // 彩蛋：仅当 props.activity 为「空闲」idle 时，按设置间隔随机播放（不连续重复同一个）。
  const lastEggRef = useRef<AtollActivity | null>(null);
  useEffect(() => {
    if (motionPaused || prefersReducedMotion() || activity !== "idle") {
      setPlayAct(null);
      return;
    }
    const intervalMs = idleIntervalSec * 1000;
    const durationMs = idleDurationSec * 1000;
    let cancelled = false;
    let timer: number;

    const pickNext = () => {
      const pool = IDLE_EASTER_EGG_ACTIVITIES;
      let next = lastEggRef.current;
      while (pool.length > 1 && next === lastEggRef.current) {
        next = pool[Math.floor(Math.random() * pool.length)];
      }
      return next;
    };

    const playOnce = () => {
      if (cancelled) return;
      const jitter = intervalMs * 0.3;
      timer = window.setTimeout(() => {
        if (cancelled) return;
        const next = pickNext();
        lastEggRef.current = next;
        setPlayAct(next);
        const loopHold = Math.max(1200, durationMs - ATOLL_ENTER_MS - ATOLL_EXIT_MS);
        timer = window.setTimeout(() => {
          if (cancelled) return;
          setPlayAct(null);
          timer = window.setTimeout(playOnce, intervalMs - jitter + Math.random() * jitter * 2);
        }, ATOLL_ENTER_MS + loopHold);
      }, intervalMs - jitter + Math.random() * jitter * 2);
    };

    playOnce();
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [activity, idleIntervalSec, idleDurationSec, motionPaused]);

  // 眨眼：普通 130ms，偶尔双眨（活泼）或慢眨（慵懒）。
  useEffect(() => {
    if (motionPaused || prefersReducedMotion()) {
      setBlinking(false);
      return;
    }
    const skipBlink =
      renderAct === "napping" || renderAct === "dead" || renderAct === "coffee" || renderAct === "music";
    if (skipBlink) return;
    let cancelled = false;
    let timer = 0;
    const wait = (ms: number, fn: () => void) => {
      timer = window.setTimeout(() => {
        if (!cancelled) fn();
      }, ms);
    };
    const blinkOnce = (closeMs: number, onDone: () => void) => {
      setBlinking(true);
      wait(closeMs, () => {
        setBlinking(false);
        wait(90, onDone);
      });
    };
    const loop = () => {
      const roll = Math.random();
      const nextDelay = () => wait(2800 + Math.random() * 2800, loop);
      if (roll < 0.12) {
        blinkOnce(130, () => blinkOnce(130, nextDelay));
      } else if (roll < 0.24) {
        blinkOnce(300, nextDelay);
      } else {
        blinkOnce(130, nextDelay);
      }
    };
    timer = window.setTimeout(loop, 2000 + Math.random() * 1500);
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [renderAct, motionPaused]);

  // reading：视线沿书行往复扫描。
  useEffect(() => {
    if (renderAct !== "reading" || prefersReducedMotion()) {
      setScanX(0);
      return;
    }
    let timer: number;
    let dir = 1;
    let pos = 0;
    const tick = () => {
      pos += dir * 2;
      if (pos >= 4) {
        pos = 4;
        dir = -1;
      } else if (pos <= -4) {
        pos = -4;
        dir = 1;
      }
      setScanX(pos);
      timer = window.setTimeout(tick, 55);
    };
    timer = window.setTimeout(tick, 200);
    return () => window.clearTimeout(timer);
  }, [renderAct]);

  // 微动作：idle 循环中每 20–45s 随机插入一个一次性小动作（不连续重复）。
  useEffect(() => {
    if (microMotion !== null) return;
    if (motionPaused || prefersReducedMotion() || renderAct !== "idle" || phase !== "loop") {
      setMicro(null);
      return;
    }
    let cancelled = false;
    let timer: number;
    const play = (last: AtollMicroMotion | null) => {
      timer = window.setTimeout(() => {
        if (cancelled) return;
        let next = last;
        while (next === last) {
          next = ATOLL_MICRO_MOTIONS[Math.floor(Math.random() * ATOLL_MICRO_MOTIONS.length)];
        }
        setMicro(next);
        timer = window.setTimeout(() => {
          if (cancelled) return;
          setMicro(null);
          play(next);
        }, ATOLL_MICRO_MS);
      }, 20000 + Math.random() * 25000);
    };
    play(null);
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [renderAct, phase, motionPaused, microMotion]);

  // 一次性状态反应。
  useEffect(() => {
    if (!reaction || motionPaused || prefersReducedMotion()) {
      setReactionActive(null);
      return;
    }
    setReactionActive(reaction);
    const timer = window.setTimeout(() => setReactionActive(null), ATOLL_REACTION_MS[reaction]);
    return () => window.clearTimeout(timer);
  }, [reaction, reactionKey, motionPaused]);

  const eyes = ACTIVITY_EYES[renderAct];
  const activeMicro = microMotion ?? micro;
  const showBlinking = blinking && renderAct !== "napping";

  const wrapperStyle = size ? { width: size * ASPECT, height: size } : undefined;

  const bodyEl = (
    <MascotBody
      baseVariant={eyes.base}
      altVariants={eyes.alt}
      blinking={showBlinking}
      eyeOffsetX={renderAct === "reading" ? scanX : 0}
    />
  );

  return (
    <span
      className={`atoll-logo is-${renderAct} is-phase-${phase}${blinking ? " is-blinking" : ""}${activeMicro ? ` is-micro-${activeMicro}` : ""}${reactionActive ? ` is-reaction-${reactionActive}` : ""}${className ? ` ${className}` : ""}`}
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
        <defs>
          <linearGradient id="atoll-meteor-grad" x1="0" y1="0" x2="1" y2="0">
            <stop offset="0" stopColor="#FFF7D6" stopOpacity="0" />
            <stop offset="1" stopColor="#FFF7D6" stopOpacity="0.9" />
          </linearGradient>
        </defs>
        <g className="atoll-stage">
          {/* ── props behind body ── */}
          {renderAct === "idea" && (
            <g className="atoll-prop atoll-bulb">
              <circle className="atoll-bulb-glow" cx="32" cy="-15" r="19" fill="#FFE566" opacity="0.16" shapeRendering="auto" />
              <rect className="atoll-ray atoll-ray-0" x="10" y="-22" width="7" height="3" fill="#FFE566" />
              <rect className="atoll-ray atoll-ray-1" x="47" y="-22" width="7" height="3" fill="#FFE566" />
              <rect className="atoll-ray atoll-ray-2" x="28" y="-34" width="4" height="7" fill="#FFE566" />
              <rect x="20" y="-28" width="24" height="26" rx="7" fill="#FFE566" shapeRendering="auto" />
              <rect x="24" y="-4" width="16" height="5" fill="#D4C45A" />
            </g>
          )}

          {renderAct === "fishing" && (
            <>
              <g className="atoll-prop atoll-water">
                <rect x="44" y="60" width="36" height="5" fill="#1F5F7A" />
                <rect x="44" y="63" width="36" height="3" fill="#2A7D9E" />
                <ellipse className="atoll-ripple" cx="72" cy="62" rx="7" ry="2" fill="none" stroke="#7FD8F0" strokeWidth="1" shapeRendering="auto" />
                <ellipse className="atoll-ripple r2" cx="72" cy="62" rx="7" ry="2" fill="none" stroke="#7FD8F0" strokeWidth="1" shapeRendering="auto" />
              </g>
              <g transform="translate(63,57)">
                <g className="atoll-fish">
                  <g className="atoll-fish-inner">
                    <rect x="0" y="0" width="9" height="5" fill="#FF8A50" />
                    <polygon points="9,0 14,2.5 9,5" fill="#FF8A50" />
                    <rect x="2" y="1" width="1.6" height="1.6" fill="#1a1a1a" />
                  </g>
                </g>
              </g>
            </>
          )}

          {renderAct === "stargazing" && (
            <g className="atoll-prop atoll-sky">
              <circle className="atoll-moon-glow" cx="-4" cy="-22" r="11.5" fill="#F0E6C8" shapeRendering="auto" />
              <circle cx="-4" cy="-22" r="7.5" fill="#F0E6C8" shapeRendering="auto" />
              <circle cx="-6.5" cy="-24" r="1.6" fill="#D9CBA4" />
              <circle cx="-1" cy="-20" r="1.1" fill="#D9CBA4" />
              <SkyStar x={10} y={-28} scale={1} cls="s1" />
              <SkyStar x={34} y={-31} scale={0.85} cls="s2" />
              <SkyStar x={58} y={-24} scale={0.7} cls="s3" />
              <SkyStar x={68} y={-6} scale={0.6} cls="s4" />
              <g transform="translate(66,-28)">
                <g className="atoll-meteor">
                  <rect x="2" y="-1" width="15" height="2" fill="url(#atoll-meteor-grad)" />
                  <circle cx="0" cy="0" r="2.2" fill="#FFF7D6" shapeRendering="auto" />
                </g>
              </g>
            </g>
          )}

          {renderAct === "music" && (
            <>
              <text className="atoll-note n1" x="-2" y="4" fontSize="13" fill="#7FD8F0" fontFamily="var(--font-mono, monospace)" shapeRendering="auto">♪</text>
              <text className="atoll-note n2" x="60" y="-2" fontSize="15" fill="#FFD24A" fontFamily="var(--font-mono, monospace)" shapeRendering="auto">♫</text>
              <text className="atoll-note n3" x="66" y="16" fontSize="11" fill="#F49AC1" fontFamily="var(--font-mono, monospace)" shapeRendering="auto">♪</text>
              <text className="atoll-note n4" x="-12" y="14" fontSize="12" fill="#9BF0C0" fontFamily="var(--font-mono, monospace)" shapeRendering="auto">♫</text>
            </>
          )}

          {renderAct === "thinking" && (
            <g className="atoll-prop atoll-thought">
              <circle cx="52" cy="2" r="3.5" fill="white" fillOpacity="0.95" />
              <circle cx="57" cy="-5" r="2.5" fill="white" fillOpacity="0.95" />
              <rect x="26" y="-30" width="38" height="22" rx="9" fill="white" fillOpacity="0.95" shapeRendering="auto" />
              <text className="atoll-think-mark atoll-think-q" x="33" y="-13" fontSize="18" fontWeight="800" fill="#7C6BC4" fontFamily="var(--font-mono, monospace)">?</text>
              <text className="atoll-think-mark atoll-think-ex" x="33" y="-13" fontSize="19" fontWeight="800" fill="#5FBF7A" fontFamily="var(--font-mono, monospace)">!</text>
              <g className="atoll-think-mark atoll-think-dots" fill="#7C6BC4">
                <circle className="atoll-dot atoll-dot-0" cx="44" cy="-17" r="2.5" />
                <circle className="atoll-dot atoll-dot-1" cx="50" cy="-17" r="2.5" />
                <circle className="atoll-dot atoll-dot-2" cx="56" cy="-17" r="2.5" />
              </g>
            </g>
          )}

          {/* ── body ── */}
          {renderAct === "music" ? (
            <g className="atoll-body-outer">
              <g className="atoll-body-mid">{bodyEl}</g>
            </g>
          ) : (
            bodyEl
          )}

          {/* ── props in front of body ── */}
          {renderAct === "coding" && (
            <g className="atoll-prop atoll-desk">
              <rect x="14" y="58" width="36" height="3" fill="#5A5A6A" />
              <rect x="16" y="61" width="3" height="7" fill="#4A4A5A" />
              <rect x="45" y="61" width="3" height="7" fill="#4A4A5A" />
              <rect x="18" y="40" width="28" height="18" fill="#2A2A3A" />
              <rect className="atoll-screen" x="20" y="42" width="24" height="14" fill="#0D1117" />
              <rect className="atoll-code-line atoll-code-0" x="22" y="45" width="10" height="1.5" fill="#6BE088" />
              <rect className="atoll-code-line atoll-code-1" x="22" y="48" width="16" height="1.5" fill="#68B8F8" />
              <rect className="atoll-code-line atoll-code-2" x="22" y="51" width="8" height="1.5" fill="#F8C868" />
              <rect className="atoll-code-line atoll-code-3" x="22" y="54" width="12" height="1.5" fill="#E080C0" />
              <text className="atoll-code-tag" x="34" y="55" fontSize="7" fontFamily="var(--font-mono, monospace)" fontWeight="700" fill="#6BE088">{"</>"}</text>
              <rect className="atoll-cursor" x="22" y="54" width="1.5" height="2" fill="#58A6FF" />
              <rect x="16" y="56" width="32" height="2" fill="#3A3A4A" />
              <rect className="atoll-key" x="18" y="57" width="5" height="2" fill="#6A6A7A" />
              <rect className="atoll-key" x="24" y="57" width="5" height="2" fill="#6A6A7A" />
              <rect className="atoll-key" x="30" y="57" width="5" height="2" fill="#6A6A7A" />
              <rect className="atoll-key" x="36" y="57" width="5" height="2" fill="#6A6A7A" />
              <rect className="atoll-key" x="42" y="57" width="5" height="2" fill="#6A6A7A" />
            </g>
          )}

          {renderAct === "reading" && (
            <g className="atoll-prop atoll-book">
              <rect className="atoll-book-cover" x="14" y="44" width="36" height="3" fill="#8B4513" />
              <rect className="atoll-book-page atoll-book-left" x="14" y="47" width="15" height="20" fill="#FFF8EC" />
              <rect className="atoll-book-page atoll-book-right" x="35" y="47" width="15" height="20" fill="#FFF8EC" />
              <rect x="27" y="45" width="10" height="24" fill="#6B3410" />
              <rect x="29" y="43" width="6" height="6" fill="#E74C3C" />
              <rect className="atoll-book-line atoll-book-line-0" x="17" y="52" width="10" height="1.5" fill="#C4A882" />
              <rect className="atoll-book-line atoll-book-line-1" x="17" y="56" width="11" height="1.5" fill="#C4A882" />
              <rect className="atoll-book-line atoll-book-line-2" x="17" y="60" width="9" height="1.5" fill="#C4A882" />
              <rect className="atoll-book-line atoll-book-line-3" x="38" y="52" width="10" height="1.5" fill="#C4A882" />
              <rect className="atoll-book-line atoll-book-line-4" x="38" y="56" width="8" height="1.5" fill="#C4A882" />
              <rect className="atoll-book-thumb" x="12" y="54" width="4" height="3" fill="#2A8FA8" opacity="0.85" />
              <rect className="atoll-book-thumb atoll-book-thumb-r" x="48" y="54" width="4" height="3" fill="#2A8FA8" opacity="0.85" />
            </g>
          )}

          {renderAct === "coffee" && (
            <g className="atoll-prop atoll-coffee-stand">
              <rect x="50" y="54" width="16" height="3" fill="#8B7355" />
              <rect x="52" y="57" width="2" height="6" fill="#6B5344" />
              <rect x="62" y="57" width="2" height="6" fill="#6B5344" />
              <g className="atoll-mug-group">
                <rect className="atoll-mug" x="52" y="38" width="12" height="16" fill="#F5F0E8" />
                <rect x="54" y="40" width="8" height="11" fill="#6B4226" />
                <rect x="64" y="42" width="4" height="3" fill="#F5F0E8" />
                <rect className="atoll-steam atoll-steam-0" x="54" y="30" width="3" height="6" rx="1" fill="white" fillOpacity="0.7" shapeRendering="auto" />
                <rect className="atoll-steam atoll-steam-1" x="60" y="28" width="3" height="7" rx="1" fill="white" fillOpacity="0.55" shapeRendering="auto" />
              </g>
              <rect className="atoll-coffee-grip" x="48" y="42" width="3" height="5" fill="#2A8FA8" opacity="0.7" />
            </g>
          )}

          {renderAct === "slacking" && (
            <>
              <g className="atoll-prop atoll-phone">
                <rect x="10" y="58" width="16" height="20" rx="2" fill="#1A1A2A" shapeRendering="auto" />
                <rect x="12" y="60" width="12" height="14" fill="#2563EB" />
                <rect className="atoll-phone-scroll" x="14" y="64" width="8" height="1.5" fill="#FFFFFF" opacity="0.9" />
                <rect className="atoll-phone-scroll atoll-phone-scroll-2" x="14" y="67" width="6" height="1.5" fill="#FFFFFF" opacity="0.65" />
                <rect className="atoll-phone-grip" x="8" y="66" width="3" height="4" fill="#2A8FA8" opacity="0.75" />
              </g>
              <g className="atoll-prop atoll-sunglasses">
                <rect x="12" y="22" width="40" height="3" fill="#1A1A1A" />
                <rect x="12" y="22" width="16" height="11" rx="2" fill="#1A1A1A" shapeRendering="auto" />
                <rect x="36" y="22" width="16" height="11" rx="2" fill="#1A1A1A" shapeRendering="auto" />
                <rect x="14" y="24" width="12" height="6" fill="#2A4060" />
                <rect x="38" y="24" width="12" height="6" fill="#2A4060" />
              </g>
            </>
          )}

          {renderAct === "napping" && (
            <>
              <g className="atoll-prop atoll-sleep-cap">
                <polygon points="22,16 54,16 58,-6" fill="#6B5B9A" />
                <rect x="20" y="14" width="36" height="5" fill="#8070B0" />
                <circle className="atoll-cap-pom" cx="60" cy="-8" r="4" fill="#E8E0F0" shapeRendering="auto" />
              </g>
              <g className="atoll-prop atoll-zzz" fill="#aab4ff" fontFamily="var(--font-mono, monospace)" fontWeight="700">
                <text className="atoll-z atoll-z-0" x="48" y="8" fontSize="12">z</text>
                <text className="atoll-z atoll-z-1" x="56" y="-4" fontSize="15">z</text>
                <text className="atoll-z atoll-z-2" x="64" y="-18" fontSize="18">z</text>
              </g>
            </>
          )}

          {renderAct === "fishing" && (
            <>
              <text className="atoll-exclaim" x="21" y="-6" fontSize="19" fontWeight="800" fill="#FFD24A" fontFamily="var(--font-mono, monospace)" shapeRendering="auto">!</text>
              <g className="atoll-rod">
                <line x1="50" y1="24" x2="72" y2="-2" stroke="#8B5A2B" strokeWidth="2.5" strokeLinecap="round" shapeRendering="auto" />
                <rect x="47" y="21" width="4" height="5" rx="1" fill="#5F707C" shapeRendering="auto" />
              </g>
              <g className="atoll-line">
                <line x1="72" y1="-1" x2="72" y2="51" stroke="#CFD8DD" strokeWidth="1" />
                <g className="atoll-bobber">
                  <rect x="70" y="51" width="4" height="2" fill="#E05555" />
                  <rect x="70" y="53" width="4" height="2" fill="#F5F0E8" />
                </g>
              </g>
              <g className="atoll-splash">
                <rect x="63" y="52" width="2" height="2" fill="#7FD8F0" />
                <rect x="68" y="51" width="2" height="2" fill="#7FD8F0" />
                <rect x="73" y="52" width="2" height="2" fill="#7FD8F0" />
              </g>
            </>
          )}

          {renderAct === "garden" && (
            <>
              <g className="atoll-plant">
                <rect x="29" y="58" width="15" height="4" rx="1" fill="#6B4226" shapeRendering="auto" />
                <g className="atoll-sprout">
                  <rect x="35.3" y="49" width="2.4" height="9" fill="#4CAF50" />
                </g>
                <g transform="rotate(-24 35.5 51)">
                  <rect className="atoll-leaf atoll-leaf-l" x="29.5" y="50.8" width="5.5" height="2.4" rx="1.2" fill="#66BB6A" shapeRendering="auto" />
                </g>
                <g transform="rotate(22 37.5 50)">
                  <rect className="atoll-leaf atoll-leaf-r" x="37.5" y="48.5" width="5.5" height="2.4" rx="1.2" fill="#66BB6A" shapeRendering="auto" />
                </g>
              </g>
              <g className="atoll-can">
                <rect x="54" y="35" width="12" height="10" rx="1" fill="#7E8E9C" shapeRendering="auto" />
                <rect x="56" y="32.5" width="8" height="2.5" rx="1.2" fill="#66757F" shapeRendering="auto" />
                <line x1="54.5" y1="38" x2="45.5" y2="42" stroke="#66757F" strokeWidth="2.5" strokeLinecap="round" shapeRendering="auto" />
                <rect x="55" y="43" width="3" height="4.5" fill="#2A8FA8" opacity="0.7" />
              </g>
              <g className="atoll-drops">
                <g className="atoll-drop">
                  <rect x="44.6" y="49" width="2.4" height="3.2" rx="1.2" fill="#8FD8F0" shapeRendering="auto" />
                </g>
                <g className="atoll-drop d2">
                  <rect x="45.6" y="50" width="2.4" height="3.2" rx="1.2" fill="#8FD8F0" shapeRendering="auto" />
                </g>
                <g className="atoll-drop d3">
                  <rect x="43.8" y="50.5" width="2.4" height="3.2" rx="1.2" fill="#8FD8F0" shapeRendering="auto" />
                </g>
              </g>
            </>
          )}

          {renderAct === "music" && (
            <>
              <text className="atoll-note atoll-note-accent" x="26" y="-16" fontSize="18" fontWeight="700" fill="#7FD8F0" fontFamily="var(--font-mono, monospace)" shapeRendering="auto">♪</text>
              <g className="atoll-phones">
                <rect x="9" y="14" width="46" height="4" rx="2" fill="#16161C" shapeRendering="auto" />
                <rect x="5" y="19" width="7" height="15" rx="2.5" fill="#16161C" shapeRendering="auto" />
                <rect x="52" y="19" width="7" height="15" rx="2.5" fill="#16161C" shapeRendering="auto" />
                <rect x="7" y="21.5" width="3" height="10" rx="1.5" fill="#3E4A58" shapeRendering="auto" />
                <rect x="54" y="21.5" width="3" height="10" rx="1.5" fill="#3E4A58" shapeRendering="auto" />
              </g>
            </>
          )}

          {renderAct === "gaming" && (
            <>
              <g className="atoll-console">
                <rect x="16" y="40" width="36" height="17" rx="2.5" fill="#2A2A3A" shapeRendering="auto" />
                <rect x="19" y="43" width="17" height="11" fill="#0D1117" />
                <rect className="atoll-ship" x="21" y="46" width="3" height="3" fill="#6BE088" />
                <rect className="atoll-enemy atoll-enemy-a" x="30.5" y="44.5" width="3" height="3" fill="#E06060" />
                <rect className="atoll-enemy atoll-enemy-b" x="31.5" y="49" width="3" height="3" fill="#E06060" />
                <rect className="atoll-laser" x="24.5" y="46.8" width="6" height="1.4" fill="#FFD24A" opacity="0" />
                <text className="atoll-win" x="20.5" y="52" fontSize="6" fontWeight="800" fill="#6BE088" fontFamily="var(--font-mono, monospace)" shapeRendering="auto">WIN</text>
                <rect className="atoll-flash" x="19" y="43" width="17" height="11" fill="#FFFFFF" opacity="0" />
                <rect x="38.5" y="46.5" width="7" height="2.2" fill="#555566" />
                <rect x="41" y="44" width="2.2" height="7" fill="#555566" />
                <circle cx="48.5" cy="46" r="2" fill="#E05555" shapeRendering="auto" />
                <circle cx="49.5" cy="51" r="2" fill="#5599E0" shapeRendering="auto" />
              </g>
              <g className="atoll-thumbs">
                <g className="atoll-thumb">
                  <rect x="21" y="56" width="4.5" height="3.5" fill="#2A8FA8" opacity="0.8" />
                </g>
                <g className="atoll-thumb t2">
                  <rect x="41" y="56" width="4.5" height="3.5" fill="#2A8FA8" opacity="0.8" />
                </g>
              </g>
              <text className="atoll-exclaim" x="25" y="-6" fontSize="19" fontWeight="800" fill="#FFD24A" fontFamily="var(--font-mono, monospace)" shapeRendering="auto">!</text>
              <g className="atoll-confetti">
                <rect x="33" y="34" width="3" height="3" fill="#FFD24A" />
                <rect x="37" y="34" width="3" height="3" fill="#9BF0C0" />
                <rect x="41" y="34" width="3" height="3" fill="#F49AC1" />
                <rect x="35" y="38" width="3" height="3" fill="#68B8F8" />
                <rect x="39" y="38" width="3" height="3" fill="#FF8A50" />
              </g>
            </>
          )}

          {/* ── 一次性反应特效 ── */}
          {reactionActive === "cheer" && (
            <g className="atoll-reaction-fx">
              <text className="atoll-check" x="24" y="-10" fontSize="22" fontWeight="800" fill="#5FBF7A" fontFamily="var(--font-mono, monospace)" shapeRendering="auto">✓</text>
              <g fill="#FFD24A">
                <path className="atoll-spark s1" d={SPARK_D} transform="translate(14,6)" />
                <path className="atoll-spark s2" d={SPARK_D} transform="translate(50,6)" />
                <path className="atoll-spark s3" d={SPARK_D} transform="translate(14,-14)" fill="#9BF0C0" />
                <path className="atoll-spark s4" d={SPARK_D} transform="translate(52,-14)" fill="#9BF0C0" />
                <path className="atoll-spark s5" d={SPARK_D} transform="translate(32,-22)" fill="#F49AC1" />
              </g>
            </g>
          )}
          {reactionActive === "revive" && (
            <g className="atoll-reaction-fx" fill="#c9d4dc">
              <circle className="atoll-puff p1" cx="4" cy="34" r="3.5" shapeRendering="auto" />
              <circle className="atoll-puff p2" cx="60" cy="34" r="3.5" shapeRendering="auto" />
            </g>
          )}
        </g>
      </svg>
    </span>
  );
}
