import { useEffect, useState } from "react";
import { ATOLL_ENTER_MS, ATOLL_EXIT_MS } from "./atollTransitions";
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
  | "napping";

interface Palette { body: string; top: string; limb: string }
const IDLE_PALETTE: Palette = { body: "#38BDD8", top: "#5FD8EC", limb: "#2A8FA8" };

const EYE = "#1a1a1a";
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
  /** Pause blink / easter-egg timers during island resize animations. */
  motionPaused?: boolean;
}

type EyeVariant = "normal" | "closed" | "happy" | "wide";

interface MascotBodyProps {
  body: string;
  top: string;
  limb: string;
  showLimbs: boolean;
  blinking: boolean;
  eyeOffsetX?: number;
  eyeVariant?: EyeVariant;
  blush?: boolean;
}

function MascotBody({
  body,
  top,
  limb,
  showLimbs,
  blinking,
  eyeOffsetX = 0,
  eyeVariant = "normal",
  blush = false,
}: MascotBodyProps) {
  const closed = eyeVariant === "closed" || blinking;
  const eyeH = closed ? 2 : eyeVariant === "wide" ? 12 : 11;
  const eyeY = closed ? EYE_Y + 4 : eyeVariant === "happy" ? EYE_Y + 5 : EYE_Y;

  return (
    <g className="atoll-body-group">
      {showLimbs && (
        <g className="atoll-limbs">
          <rect className="atoll-limb atoll-leg-left" x="20" y="44" width="6" height="12" fill={limb} />
          <rect className="atoll-limb atoll-leg-right" x="38" y="44" width="6" height="12" fill={limb} />
        </g>
      )}
      <rect x={BX} y={BY} width={BW} height={BH} fill={body} />
      <rect x={BX} y={BY} width={BW} height={BTOP} fill={top} />
      {blush && (
        <g className="atoll-blush" opacity="0.55">
          <ellipse cx="17" cy="34" rx="4" ry="2" fill="#ffb4b4" shapeRendering="auto" />
          <ellipse cx="47" cy="34" rx="4" ry="2" fill="#ffb4b4" shapeRendering="auto" />
        </g>
      )}
      <g className="atoll-eyes">
        {eyeVariant === "happy" && !blinking ? (
          <>
            <rect x={18 + eyeOffsetX} y={eyeY} width={8} height={2} fill={EYE} />
            <rect x={38 + eyeOffsetX} y={eyeY} width={8} height={2} fill={EYE} />
          </>
        ) : (
          <>
            <rect x={19 + eyeOffsetX} y={eyeY} width={5} height={eyeH} fill={EYE} />
            <rect x={38 + eyeOffsetX} y={eyeY} width={5} height={eyeH} fill={EYE} />
          </>
        )}
      </g>
    </g>
  );
}

export function AtollLogo({
  activity = "idle",
  size = 64,
  className,
  idleIntervalSec = DEFAULT_IDLE_INTERVAL_SEC,
  idleDurationSec = DEFAULT_IDLE_DURATION_SEC,
  motionPaused = false,
}: AtollLogoProps) {
  const [playAct, setPlayAct] = useState<AtollActivity | null>(null);
  const [blinking, setBlinking] = useState(false);
  const [scanX, setScanX] = useState(0);

  const targetAct = activity === "idle" ? (playAct ?? "idle") : activity;
  const { renderAct, phase } = useAtollPhase(targetAct);

  useEffect(() => {
    if (motionPaused) {
      setPlayAct(null);
      return;
    }
    // 彩蛋：仅当 props.activity 为「空闲」idle 时，按设置间隔从 IDLE_EASTER_EGG_ACTIVITIES 随机播放。
    if (activity !== "idle") {
      setPlayAct(null);
      return;
    }
    const intervalMs = idleIntervalSec * 1000;
    const durationMs = idleDurationSec * 1000;
    let cancelled = false;
    let timer: number;

    const playOnce = () => {
      if (cancelled) return;
      const jitter = intervalMs * 0.3;
      timer = window.setTimeout(() => {
        if (cancelled) return;
        setPlayAct(IDLE_EASTER_EGG_ACTIVITIES[Math.floor(Math.random() * IDLE_EASTER_EGG_ACTIVITIES.length)]);
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

  const palette = IDLE_PALETTE;
  const showLimbs = renderAct !== "idle" && renderAct !== "napping";

  useEffect(() => {
    if (motionPaused) {
      setBlinking(false);
      return;
    }
    if (renderAct === "napping") return;
    let timer: number;
    const blinkOnce = (onDone: () => void) => {
      setBlinking(true);
      window.setTimeout(() => { setBlinking(false); onDone(); }, 130);
    };
    const loop = () => {
      blinkOnce(() => {
        timer = window.setTimeout(loop, 2800 + Math.random() * 2800);
      });
    };
    timer = window.setTimeout(loop, 2000 + Math.random() * 1500);
    return () => window.clearTimeout(timer);
  }, [renderAct, motionPaused]);

  useEffect(() => {
    if (renderAct !== "reading") { setScanX(0); return; }
    let timer: number;
    let dir = 1;
    let pos = 0;
    const tick = () => {
      pos += dir * 2;
      if (pos >= 4) { pos = 4; dir = -1; }
      else if (pos <= -4) { pos = -4; dir = 1; }
      setScanX(pos);
      timer = window.setTimeout(tick, 55);
    };
    timer = window.setTimeout(tick, 200);
    return () => window.clearTimeout(timer);
  }, [renderAct]);

  const eyeVariant: EyeVariant =
    renderAct === "napping" ? "closed"
    : renderAct === "coffee" ? "happy"
    : renderAct === "idea" ? "wide"
    : "normal";

  const wrapperStyle = size ? { width: size * ASPECT, height: size } : undefined;

  return (
    <span
      className={`atoll-logo is-${renderAct} is-phase-${phase}${blinking ? " is-blinking" : ""}${className ? ` ${className}` : ""}`}
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
        {/* ── props behind body ── */}
        {renderAct === "idea" && (
          <g className="atoll-prop atoll-bulb">
            <rect className="atoll-ray atoll-ray-0" x="10" y="-22" width="7" height="3" fill="#FFE566" />
            <rect className="atoll-ray atoll-ray-1" x="47" y="-22" width="7" height="3" fill="#FFE566" />
            <rect className="atoll-ray atoll-ray-2" x="28" y="-34" width="4" height="7" fill="#FFE566" />
            <rect x="20" y="-28" width="24" height="26" rx="7" fill="#FFE566" shapeRendering="auto" />
            <rect x="24" y="-4" width="16" height="5" fill="#D4C45A" />
          </g>
        )}

        {renderAct === "napping" && (
          <g className="atoll-prop atoll-sleep-cap">
            <polygon points="22,16 54,16 58,-6" fill="#6B5B9A" />
            <rect x="20" y="14" width="36" height="5" fill="#8070B0" />
            <circle className="atoll-cap-pom" cx="60" cy="-8" r="4" fill="#E8E0F0" />
          </g>
        )}

        <MascotBody
          body={palette.body}
          top={palette.top}
          limb={palette.limb}
          showLimbs={showLimbs}
          blinking={blinking && renderAct !== "napping"}
          eyeOffsetX={renderAct === "reading" ? scanX : 0}
          eyeVariant={eyeVariant}
          blush={renderAct === "coffee" || renderAct === "idea"}
        />

        {/* ── coding: 面前小桌 + 键盘打字（隐形手） ── */}
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
            <text className="atoll-code-tag" x="34" y="55" fontSize="7" fontFamily="var(--font-mono, monospace)" fontWeight="700" fill="#6BE088">{`</>`}</text>
            <rect className="atoll-cursor" x="22" y="54" width="1.5" height="2" fill="#58A6FF" />
            <rect x="16" y="56" width="32" height="2" fill="#3A3A4A" />
            <rect className="atoll-key atoll-key-0" x="18" y="57" width="5" height="2" fill="#6A6A7A" />
            <rect className="atoll-key atoll-key-1" x="24" y="57" width="5" height="2" fill="#6A6A7A" />
            <rect className="atoll-key atoll-key-2" x="30" y="57" width="5" height="2" fill="#6A6A7A" />
            <rect className="atoll-key atoll-key-3" x="36" y="57" width="5" height="2" fill="#6A6A7A" />
            <rect className="atoll-key atoll-key-4" x="42" y="57" width="5" height="2" fill="#6A6A7A" />
          </g>
        )}

        {/* ── reading: 胸前捧书（隐形手） ── */}
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

        {/* ── thinking: 问号气泡 ── */}
        {renderAct === "thinking" && (
          <g className="atoll-prop atoll-thought">
            <circle cx="52" cy="2" r="3.5" fill="white" fillOpacity="0.95" />
            <circle cx="57" cy="-5" r="2.5" fill="white" fillOpacity="0.95" />
            <rect x="26" y="-30" width="38" height="22" rx="9" fill="white" fillOpacity="0.95" shapeRendering="auto" />
            <text className="atoll-think-mark atoll-think-q" x="34" y="-13" fontSize="18" fontWeight="800" fill="#7C6BC4" fontFamily="var(--font-mono, monospace)">?</text>
            <g className="atoll-think-mark atoll-think-dots" fill="#7C6BC4">
              <circle className="atoll-dot atoll-dot-0" cx="36" cy="-17" r="2.5" />
              <circle className="atoll-dot atoll-dot-1" cx="44" cy="-17" r="2.5" />
              <circle className="atoll-dot atoll-dot-2" cx="52" cy="-17" r="2.5" />
            </g>
          </g>
        )}

        {/* ── coffee: 举杯（隐形手） ── */}
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

        {/* ── slacking: 墨镜 + 举手机（隐形手） ── */}
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

        {/* ── napping: zzz ── */}
        {renderAct === "napping" && (
          <g className="atoll-prop atoll-zzz" fill="#aab4ff" fontFamily="var(--font-mono, monospace)" fontWeight="700">
            <text className="atoll-z atoll-z-0" x="48" y="8" fontSize="12">z</text>
            <text className="atoll-z atoll-z-1" x="56" y="-4" fontSize="15">z</text>
            <text className="atoll-z atoll-z-2" x="64" y="-18" fontSize="18">z</text>
          </g>
        )}
      </svg>
    </span>
  );
}
