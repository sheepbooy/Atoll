import {
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type MouseEvent as ReactMouseEvent,
  type PointerEvent as ReactPointerEvent,
} from "react";
import { PixelDigitDisplay } from "./PixelDigitDisplay";
import {
  buildDigitReelStrip,
  buildStaticTokenOdometerCells,
  buildTokenOdometerCells,
  formatCompactTokenCount,
  stepAnimatedTokenValue,
  tokenDisplayCompactLevel,
  type TokenOdometerCell,
} from "./tokenCounterFormat";
import { onIslandHoverChanged, type TokenUsage } from "./tauri";

type TokenCounterEnergy = "idle" | "live" | "settle";
export type TokenCounterVariant = "compact" | "expanded";

function tokenScopeLabel(variant: TokenCounterVariant): string {
  return variant === "expanded" ? "Today tokens" : "Active session tokens";
}

function tokenScopeShortLabel(variant: TokenCounterVariant): string {
  return variant === "expanded" ? "Today" : "Active";
}

function TokenScopeMark({ variant }: { variant: TokenCounterVariant }) {
  return (
    <span
      className={`token-counter-mark token-counter-mark--${variant}`}
      aria-hidden="true"
      title={tokenScopeShortLabel(variant)}
    />
  );
}

function tokenScopeHint(variant: TokenCounterVariant): string {
  return variant === "expanded"
    ? "All sessions today, including archived."
    : "Active sessions in the island right now.";
}

function tokenCounterTitle(
  value: number,
  usage: TokenUsage,
  variant: TokenCounterVariant,
): string {
  return [
    `${tokenScopeLabel(variant)} ${value.toLocaleString()}`,
    `input ${usage.inputTokens.toLocaleString()}`,
    `output ${usage.outputTokens.toLocaleString()}`,
    `cache-read ${usage.cacheReadTokens.toLocaleString()}`,
    `cache-write ${usage.cacheCreationTokens.toLocaleString()}`,
  ].join(" · ");
}

function isPointInsideRect(
  x: number,
  y: number,
  rect: DOMRect,
  padding = 3,
): boolean {
  return (
    x >= rect.left - padding &&
    x <= rect.right + padding &&
    y >= rect.top - padding &&
    y <= rect.bottom + padding
  );
}

function TokenCounterTooltip({
  value,
  usage,
  visible,
  variant,
}: {
  value: number;
  usage: TokenUsage;
  visible: boolean;
  variant: TokenCounterVariant;
}) {
  return (
    <span
      id="token-counter-tooltip"
      className={`token-counter-tooltip${visible ? " is-visible" : ""}`}
      role="tooltip"
      aria-hidden={!visible}
    >
      <span className="token-counter-tooltip-headline">
        {tokenScopeLabel(variant)} {value.toLocaleString()}
      </span>
      <span className="token-counter-tooltip-detail">
        in {usage.inputTokens.toLocaleString()} · out{" "}
        {usage.outputTokens.toLocaleString()}
      </span>
      <span className="token-counter-tooltip-detail">
        cache-read {usage.cacheReadTokens.toLocaleString()} · cache-write{" "}
        {usage.cacheCreationTokens.toLocaleString()}
      </span>
      <span className="token-counter-tooltip-hint">{tokenScopeHint(variant)}</span>
    </span>
  );
}

function rollStyle(delayMs: number, steps?: number) {
  if (delayMs <= 0 && steps === undefined) return undefined;
  return {
    animationDelay: delayMs > 0 ? `${delayMs}ms` : undefined,
    ...(steps !== undefined ? { ["--reel-steps" as string]: steps } : {}),
  } as const;
}

function TokenSlotReel({
  fromChar,
  toChar,
  delayMs,
}: {
  fromChar: string;
  toChar: string;
  delayMs: number;
}) {
  const strip = useMemo(
    () => buildDigitReelStrip(fromChar, toChar),
    [fromChar, toChar],
  );
  const steps = strip.length - 1;

  return (
    <span className="token-odo-slot token-odo-slot--reel" aria-hidden="true">
      <span className="token-odo-reel-strip" style={rollStyle(delayMs, steps)}>
        {strip.map((digit, index) => (
          <span key={`${digit}-${index}`} className="token-odo-reel-digit">
            {digit}
          </span>
        ))}
      </span>
    </span>
  );
}

function TokenSlotChar({ cell }: { cell: TokenOdometerCell }) {
  if (cell.entering) {
    return (
      <span className="token-odo-slot" aria-hidden="true">
        <span
          className="token-odo-roll token-odo-roll-enter"
          style={rollStyle(cell.rollDelayMs)}
        >
          {cell.char}
        </span>
      </span>
    );
  }

  if (cell.kind === "digit" && cell.changed && cell.prevChar) {
    return (
      <TokenSlotReel
        fromChar={cell.prevChar}
        toChar={cell.char}
        delayMs={cell.rollDelayMs}
      />
    );
  }

  return (
    <span className={`token-odo-char token-odo-char--${cell.kind}`}>
      {cell.char}
    </span>
  );
}

function TokenSlotOdometer({
  text,
  energy,
  animateDigits,
}: {
  text: string;
  energy: TokenCounterEnergy;
  animateDigits: boolean;
}) {
  const prevTextRef = useRef(text);
  const cells = useMemo(
    () =>
      animateDigits
        ? buildTokenOdometerCells(text, prevTextRef.current)
        : buildStaticTokenOdometerCells(text),
    [animateDigits, text],
  );

  useLayoutEffect(() => {
    prevTextRef.current = text;
  }, [text]);

  return (
    <span className={`token-odo token-odo--${energy}`}>
      {cells.map((cell, index) => (
        <TokenSlotChar
          key={`${index}-${cell.char}-${cell.changed ? cell.prevChar : ""}-${cell.entering ? "enter" : ""}`}
          cell={cell}
        />
      ))}
    </span>
  );
}

export interface TokenCounterProps {
  value: number;
  usage: TokenUsage;
  variant?: TokenCounterVariant;
  sessionCount?: number;
  maxCompactIcons?: number;
  /** When set, overrides width/session heuristics for collapsed display. */
  compactTokenLevel?: number;
  /** Hold digits static during island open/close animations. */
  suppressAnimations?: boolean;
}

const DEFAULT_ICON_LIMIT = 3;

export function TokenCounter({
  value,
  usage,
  variant = "compact",
  sessionCount = 0,
  maxCompactIcons = DEFAULT_ICON_LIMIT,
  compactTokenLevel,
  suppressAnimations = false,
}: TokenCounterProps) {
  const compactLevel =
    variant === "compact" && compactTokenLevel !== undefined
      ? compactTokenLevel
      : tokenDisplayCompactLevel(value, variant, sessionCount, maxCompactIcons);
  const animateDigits = !suppressAnimations;
  const [displayText, setDisplayText] = useState(() =>
    formatCompactTokenCount(value, compactLevel, value),
  );
  const [energy, setEnergy] = useState<TokenCounterEnergy>("idle");
  const [deltaText, setDeltaText] = useState<string | null>(null);
  const [deltaKey, setDeltaKey] = useState(0);
  const [tooltipVisible, setTooltipVisible] = useState(false);

  const wrapRef = useRef<HTMLSpanElement>(null);
  const pointerHoverRef = useRef(false);
  const displayTextRef = useRef(displayText);
  const animatedValueRef = useRef(value);
  const targetRef = useRef(value);
  const lastFrameAtRef = useRef<number | null>(null);
  const frameRef = useRef<number | null>(null);
  const settleTimerRef = useRef<number | null>(null);
  const deltaTimerRef = useRef<number | null>(null);

  displayTextRef.current = displayText;

  useEffect(() => {
    let unsubscribe = () => {};

    onIslandHoverChanged(({ hovering, clientX, clientY }) => {
      const wrap = wrapRef.current;
      if (!wrap) return;

      if (!hovering || clientX == null || clientY == null) {
        if (!hovering && !pointerHoverRef.current) {
          setTooltipVisible(false);
        }
        return;
      }

      const inside = isPointInsideRect(
        clientX,
        clientY,
        wrap.getBoundingClientRect(),
      );
      setTooltipVisible(inside || pointerHoverRef.current);
    }).then((cleanup) => {
      unsubscribe = cleanup;
    });

    return () => {
      unsubscribe();
    };
  }, []);

  useEffect(() => {
    const clearTimers = () => {
      if (settleTimerRef.current !== null) {
        window.clearTimeout(settleTimerRef.current);
        settleTimerRef.current = null;
      }
      if (deltaTimerRef.current !== null) {
        window.clearTimeout(deltaTimerRef.current);
        deltaTimerRef.current = null;
      }
    };

    clearTimers();

    const previousTarget = targetRef.current;
    targetRef.current = value;
    const incomingDelta = value - previousTarget;

    if (
      incomingDelta > 0 &&
      variant === "compact" &&
      !suppressAnimations
    ) {
      setDeltaText(`+${formatCompactTokenCount(incomingDelta, compactLevel, value)}`);
      setDeltaKey((key) => key + 1);
      deltaTimerRef.current = window.setTimeout(() => {
        setDeltaText(null);
        deltaTimerRef.current = null;
      }, 960);
    } else if (incomingDelta <= 0) {
      setDeltaText(null);
    }

    const publishDisplay = (nextValue: number) => {
      const rounded = Math.round(nextValue);
      const nextText = formatCompactTokenCount(rounded, compactLevel, targetRef.current);
      animatedValueRef.current = nextValue;
      if (nextText !== displayTextRef.current) {
        displayTextRef.current = nextText;
        setDisplayText(nextText);
      }
    };

    if (suppressAnimations) {
      publishDisplay(value);
      setEnergy("idle");
      return clearTimers;
    }

    if (Math.abs(value - animatedValueRef.current) < 0.5) {
      publishDisplay(value);
      return clearTimers;
    }

    setEnergy("live");

    const animate = (now: number) => {
      const previousFrameAt = lastFrameAtRef.current ?? now;
      lastFrameAtRef.current = now;
      const dt = Math.min(0.05, Math.max(0.001, (now - previousFrameAt) / 1000));
      const next = stepAnimatedTokenValue(animatedValueRef.current, targetRef.current, dt);

      publishDisplay(next);

      if (Math.abs(targetRef.current - next) < 0.5) {
        publishDisplay(targetRef.current);
        frameRef.current = null;
        lastFrameAtRef.current = null;
        setEnergy("settle");
        settleTimerRef.current = window.setTimeout(() => {
          setEnergy("idle");
          settleTimerRef.current = null;
        }, variant === "expanded" ? 820 : 640);
        return;
      }

      frameRef.current = window.requestAnimationFrame(animate);
    };

    if (frameRef.current !== null) {
      window.cancelAnimationFrame(frameRef.current);
    }
    frameRef.current = window.requestAnimationFrame(animate);

    return () => {
      if (frameRef.current !== null) {
        window.cancelAnimationFrame(frameRef.current);
        frameRef.current = null;
        lastFrameAtRef.current = null;
      }
      clearTimers();
    };
  }, [value, compactLevel, variant, suppressAnimations]);

  function handlePointerEnter() {
    pointerHoverRef.current = true;
    setTooltipVisible(true);
  }

  function handlePointerLeave() {
    pointerHoverRef.current = false;
    setTooltipVisible(false);
  }

  function handleMouseDown(event: ReactMouseEvent<HTMLSpanElement>) {
    event.stopPropagation();
  }

  function handlePointerDown(event: ReactPointerEvent<HTMLSpanElement>) {
    event.stopPropagation();
  }

  return (
    <span
      ref={wrapRef}
      className={`token-counter-wrap token-counter-wrap--${variant} token-counter-wrap--${energy}${
        suppressAnimations ? " is-present-transition" : ""
      }`}
      data-no-drag
      onMouseDown={handleMouseDown}
      onPointerDown={handlePointerDown}
      onPointerEnter={handlePointerEnter}
      onPointerLeave={handlePointerLeave}
    >
      <TokenCounterTooltip
        value={value}
        usage={usage}
        visible={tooltipVisible}
        variant={variant}
      />
      <span className={`token-counter-body token-counter-body--${variant}`}>
        <TokenScopeMark variant={variant} />
        <span
          className="token-counter"
          aria-label={tokenCounterTitle(value, usage, variant)}
          aria-describedby={tooltipVisible ? "token-counter-tooltip" : undefined}
        >
          {variant === "expanded" ? (
            <PixelDigitDisplay
              text={displayText}
              energy={energy}
              animateDigits={animateDigits}
            />
          ) : (
            <TokenSlotOdometer
              text={displayText}
              energy={energy}
              animateDigits={animateDigits}
            />
          )}
        </span>
      </span>
      {deltaText ? (
        <span key={deltaKey} className="token-counter-delta" aria-hidden="true">
          {deltaText}
        </span>
      ) : null}
    </span>
  );
}
