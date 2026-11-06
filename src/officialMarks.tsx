import { useId } from "react";
import type { ClawdMood } from "./mascotShared";
import type { MascotTimeline } from "./mascotMotion";
import { GeminiContourAnimation } from "./geminiMotion";

/** Official Codex mark: OpenAI blossom with a `>_` prompt cutout. */
export const CODEX_OFFICIAL_PATH =
  "M8.086.457a6.105 6.105 0 013.046-.415c1.333.153 2.521.72 3.564 1.7a.117.117 0 00.107.029c1.408-.346 2.762-.224 4.061.366l.063.03.154.076c1.357.703 2.33 1.77 2.918 3.198.278.679.418 1.388.421 2.126a5.655 5.655 0 01-.18 1.631.167.167 0 00.04.155 5.982 5.982 0 011.578 2.891c.385 1.901-.01 3.615-1.183 5.14l-.182.22a6.063 6.063 0 01-2.934 1.851.162.162 0 00-.108.102c-.255.736-.511 1.364-.987 1.992-1.199 1.582-2.962 2.462-4.948 2.451-1.583-.008-2.986-.587-4.21-1.736a.145.145 0 00-.14-.032c-.518.167-1.04.191-1.604.185a5.924 5.924 0 01-2.595-.622 6.058 6.058 0 01-2.146-1.781c-.203-.269-.404-.522-.551-.821a7.74 7.74 0 01-.495-1.283 6.11 6.11 0 01-.017-3.064.166.166 0 00.008-.074.115.115 0 00-.037-.064 5.958 5.958 0 01-1.38-2.202 5.196 5.196 0 01-.333-1.589 6.915 6.915 0 01.188-2.132c.45-1.484 1.309-2.648 2.577-3.493.282-.188.55-.334.802-.438.286-.12.573-.22.861-.304a.129.129 0 00.087-.087A6.016 6.016 0 015.635 2.31C6.315 1.464 7.132.846 8.086.457zm-.804 7.85a.848.848 0 00-1.473.842l1.694 2.965-1.688 2.848a.849.849 0 001.46.864l1.94-3.272a.849.849 0 00.007-.854l-1.94-3.393zm5.446 6.24a.849.849 0 000 1.695h4.848a.849.849 0 000-1.696h-4.848z";

/** Official Cursor 2.5D cube (cursor.com/brand), original viewBox 395 393 175 197. */
export const CURSOR_CUBE_PATHS = {
  bottom: "M483.395 490.5L566 538.297C565.493 539.178 564.757 539.93 563.845 540.456L486.636 585.13C484.632 586.29 482.159 586.29 480.154 585.13L402.945 540.456C402.034 539.93 401.297 539.178 400.79 538.297L483.395 490.5Z",
  left: "M483.395 395V490.5L400.79 538.297C400.282 537.416 400 536.398 400 535.346V445.654C400 443.545 401.122 441.6 402.945 440.544L480.15 395.87C481.154 395.29 482.273 395 483.391 395H483.395Z",
  right: "M565.996 442.703C565.489 441.822 564.752 441.07 563.841 440.544L486.632 395.87C485.632 395.29 484.513 395 483.395 395V490.5L566 538.297C566.507 537.416 566.789 536.398 566.789 535.346V445.654C566.789 444.598 566.511 443.588 566 442.703H565.996Z",
  stem: "M560.218 446.049C560.686 446.858 560.751 447.896 560.218 448.82L485.235 578.974C484.732 579.855 483.392 579.493 483.392 578.479V492.713C483.392 492.029 483.209 491.37 482.877 490.794L560.215 446.045H560.218V446.049Z",
  head: "M560.218 446.049L482.88 490.797C482.552 490.224 482.073 489.737 481.48 489.394L407.369 446.511C406.49 446.006 406.851 444.663 407.862 444.663H557.824C558.889 444.663 559.754 445.239 560.218 446.049Z",
} as const;

export interface CursorCubePalette {
  bottom: string;
  left: string;
  right: string;
  stem: string;
  head: string;
}

export const CURSOR_CUBE_OFFICIAL: CursorCubePalette = {
  bottom: "#72716D",
  left: "#55544F",
  right: "#43413C",
  stem: "#D6D5D2",
  head: "#FFFFFF",
};

export const CURSOR_CUBE_DEAD: CursorCubePalette = {
  bottom: "#7a7a7a",
  left: "#5c5c5c",
  right: "#484848",
  stem: "#c8c8c8",
  head: "#e8e8e8",
};

export const CURSOR_CUBE_SICK: CursorCubePalette = {
  bottom: "#6d8a6d",
  left: "#4f6b4f",
  right: "#3c523c",
  stem: "#c5d6c5",
  head: "#f4fff4",
};

export function cursorCubePalette(mood: ClawdMood): CursorCubePalette {
  if (mood === "dead") return CURSOR_CUBE_DEAD;
  if (mood === "worried") return CURSOR_CUBE_SICK;
  return CURSOR_CUBE_OFFICIAL;
}

export function codexMarkFill(mood: ClawdMood, accent?: string): string {
  if (mood === "dead") return "#8a8a8a";
  if (mood === "worried") return "#7cb97c";
  if (mood === "sleeping") return accent || "#a8b0b4";
  return accent || "#f4f4f4";
}

// Preserve the official silhouette and the exact two prompt cutouts at rest.
const CODEX_SHELL_PATH = CODEX_OFFICIAL_PATH.split("zm")[0] + "z";
const CODEX_CHEVRON_PATH = "M7.282 8.307a.848.848 0 00-1.473.842l1.694 2.965-1.688 2.848a.849.849 0 001.46.864l1.94-3.272a.849.849 0 00.007-.854l-1.94-3.393z";
const CODEX_CARET_PATH = "M12.728 14.547a.849.849 0 000 1.695h4.848a.849.849 0 000-1.696h-4.848z";
const PETAL_SECTORS = Array.from({ length: 6 }, (_, i) => {
  const angle = (i * 60 - 120) * Math.PI / 180;
  const point = (r: number, a: number) => `${12.15 + r * Math.cos(a)},${12.15 + r * Math.sin(a)}`;
  return `12.15,12.15 ${point(40, angle - 0.004)} ${point(40, angle + Math.PI / 3 + 0.004)}`;
});

export function CodexOfficialMark({ fill, className }: { fill: string; className?: string }) {
  const id = `codex-parts-${useId().replace(/[^a-zA-Z0-9]/g, "")}`;
  return <g className={className}>
    <g transform="translate(56 26.4) scale(3.5) translate(-12.15 -8)">
      <defs>
        <mask id={`${id}-prompt`} maskUnits="userSpaceOnUse" x="-8" y="-8" width="40" height="40" style={{ maskType: "luminance" }}>
          <rect x="-8" y="-8" width="40" height="40" fill="white" />
          <g data-part="prompt" className="codex-prompt"><path d={CODEX_CHEVRON_PATH} fill="black" /></g>
          <g data-part="caret" className="codex-caret"><path d={CODEX_CARET_PATH} fill="black" /></g>
        </mask>
        <clipPath id={`${id}-hub`}><circle cx="12.15" cy="12.15" r="6.5" /></clipPath>
        {PETAL_SECTORS.map((points, i) => <clipPath id={`${id}-petal-${i}`} key={i}><polygon points={points} /></clipPath>)}
      </defs>
      <path className="mascot-original" fill={fill} fillRule="evenodd" clipRule="evenodd" d={CODEX_OFFICIAL_PATH} />
      <g className="mascot-articulated" mask={`url(#${id}-prompt)`}>
        <g data-part="shell" className="codex-shell"><path fill={fill} d={CODEX_SHELL_PATH} /></g>
        {PETAL_SECTORS.map((_, i) => <g key={i} data-part={`petal-${i}`} className={`codex-petal codex-petal-${i}`}>
          <path fill={fill} d={CODEX_SHELL_PATH} clipPath={`url(#${id}-petal-${i})`} />
        </g>)}
        <path fill={fill} d={CODEX_SHELL_PATH} clipPath={`url(#${id}-hub)`} />
      </g>
    </g>
  </g>;
}

/** ZCode mark: rounded tile with a bold italic "Z" after the official app icon.
 * The tile uses the agent's sky-blue brand gradient so it reads on both the dark
 * island panel and light surfaces without needing an outline. */
export const ZCODE_TILE_PATH =
  "M5.3 .6h13.4a5.3 5.3 0 015.3 5.3v12.2a5.3 5.3 0 01-5.3 5.3H5.3A5.3 5.3 0 010 18.1V5.9A5.3 5.3 0 015.3 .6Z";

export const ZCODE_Z_PATH = "M5.2 5.8H18.8V8L9.2 16H18.8V18.2H5.2V16L14.8 8H5.2Z";

export function zcodeTileGradientStops(
  mood: ClawdMood,
  accent?: string,
  accentDark?: string,
): { from: string; to: string } {
  if (mood === "dead") return { from: "#55585e", to: "#3a3d42" };
  if (mood === "worried") return { from: "#4a5a66", to: "#35434d" };
  // Session rows tint the tile with the per-session palette color; agent tabs
  // keep the brand gradient by not passing an accent.
  if (accent) return { from: accent, to: accentDark || accent };
  if (mood === "sleeping") return { from: "#3f6379", to: "#28455a" };
  return { from: "#58c7f5", to: "#1f8fd0" };
}

export function zcodeMarkFill(mood: ClawdMood): string {
  if (mood === "dead") return "#c8ccd2";
  if (mood === "worried") return "#7cb97c";
  if (mood === "sleeping") return "#dbe7ee";
  return "#ffffff";
}

export function ZcodeOfficialMark({
  mood,
  accent,
  accentDark,
  className,
}: {
  mood: ClawdMood;
  accent?: string;
  accentDark?: string;
  className?: string;
}) {
  // Per-instance gradient id: several mascots with different moods (e.g. a dead
  // header logo next to a live agent tab) must not share one gradient def.
  const gradientId = `zcode-tile-${useId().replace(/[^a-zA-Z0-9]/g, "")}`;
  const { from, to } = zcodeTileGradientStops(mood, accent, accentDark);

  return (
    <g className={className}>
      <defs>
        <linearGradient id={gradientId} x1="0" y1="0" x2="0.35" y2="1">
          <stop offset="0" stopColor={from} />
          <stop offset="1" stopColor={to} />
        </linearGradient>
      </defs>
      {/* Same visual band as the Codex blossom / Cursor cube (center ≈ y 37-40). */}
      <g transform="translate(56 39) scale(3.15) translate(-12 -12)">
        <g data-part="tile" className="zcode-tile"><path fill={`url(#${gradientId})`} d={ZCODE_TILE_PATH} /></g>
        <g transform="translate(2 0) skewX(-9.5)">
          <g data-part="letter" className="zcode-letter">
            <path data-part="letter-base" className="zcode-letter-base" fill={zcodeMarkFill(mood)} d={ZCODE_Z_PATH} />
            <defs>
              <clipPath id={`${gradientId}-top`}><rect x="4" y="5" width="16" height="3" /></clipPath>
              <clipPath id={`${gradientId}-diagonal`}><rect x="4" y="8" width="16" height="8" /></clipPath>
              <clipPath id={`${gradientId}-bottom`}><rect x="4" y="16" width="16" height="3" /></clipPath>
            </defs>
            {["top", "diagonal", "bottom"].map((part) => <g key={part} className={`zcode-ink zcode-ink-${part}`} data-part={`ink-${part}`}>
              <path fill={zcodeMarkFill(mood)} d={ZCODE_Z_PATH} clipPath={`url(#${gradientId}-${part})`} />
            </g>)}
          </g>
        </g>
      </g>
    </g>
  );
}

/** Official Gemini spark: the four-point star from Google's Gemini brand,
 * filled with the documented three-stop gradient (blue → purple → rose).
 * Path data from the official mark (24×24, tips on the axes). */
export const GEMINI_SPARK_PATH =
  "M11.04 19.32Q12 21.51 12 24q0-2.49.93-4.68.96-2.19 2.58-3.81t3.81-2.55Q21.51 12 24 12q-2.49 0-4.68-.93a12.3 12.3 0 0 1-3.81-2.58 12.3 12.3 0 0 1-2.58-3.81Q12 2.49 12 0q0 2.49-.96 4.68-.93 2.19-2.55 3.81a12.3 12.3 0 0 1-3.81 2.58Q2.49 12 0 12q2.49 0 4.68.96 2.19.93 3.81 2.55t2.55 3.81";

export const GEMINI_BRAND_GRADIENT = { from: "#4796E3", mid: "#9177C7", to: "#CA6673" };

export function geminiSparkGradientStops(
  mood: ClawdMood,
  accent?: string,
  accentDark?: string,
): { from: string; mid?: string; to: string } {
  if (mood === "dead") return { from: "#8a8a8a", to: "#666666" };
  if (mood === "worried") return { from: "#7cb97c", to: "#5a8b5a" };
  if (mood === "sleeping") return { from: "#6f7f95", to: "#495a72" };
  // Session rows tint the spark with the per-session palette color; agent
  // tabs keep the brand gradient by not passing an accent.
  if (accent) return { from: accent, to: accentDark || accent };
  return GEMINI_BRAND_GRADIENT;
}

export function GeminiOfficialMark({
  mood,
  accent,
  accentDark,
  className,
  timeline,
}: {
  mood: ClawdMood;
  accent?: string;
  accentDark?: string;
  className?: string;
  timeline?: MascotTimeline | null;
}) {
  // Per-instance gradient id: several mascots with different moods (e.g. a dead
  // header logo next to a live agent tab) must not share one gradient def.
  const gradientId = `gemini-spark-${useId().replace(/[^a-zA-Z0-9]/g, "")}`;
  const { from, mid, to } = geminiSparkGradientStops(mood, accent, accentDark);

  return (
    <g className={className}>
      <defs>
        <linearGradient id={gradientId} gradientUnits="userSpaceOnUse" x1="0" y1="0" x2="21.6" y2="24">
          <stop offset="0" stopColor={from} />
          {mid !== undefined && <stop offset="0.5" stopColor={mid} />}
          <stop offset="1" stopColor={to} />
        </linearGradient>
      </defs>
      {/* Same visual band as the Codex blossom / Cursor cube / ZCode tile. */}
      <g transform="translate(56 39) scale(3.15) translate(-12 -12)">
        <defs>
          {["12,12 -40,-40 64,-40", "12,12 64,-40 64,64", "12,12 64,64 -40,64", "12,12 -40,64 -40,-40"].map((points, i) =>
            <clipPath id={`${gradientId}-tip-${i}`} key={i}><polygon points={points} /></clipPath>)}
        </defs>
        <path className="mascot-original" fill={`url(#${gradientId})`} d={GEMINI_SPARK_PATH} />
        <g className="mascot-articulated">
          <g data-part="core" className="gemini-core"><path fill={`url(#${gradientId})`} d={GEMINI_SPARK_PATH}>
            <GeminiContourAnimation key={`${timeline?.key}:${timeline?.durationMs}`} timeline={timeline} />
          </path></g>
          {[0, 1, 2, 3].map(i => <g key={i} data-part={`tip-${i}`} className={`gemini-tip gemini-tip-${i}`}>
            <path fill={`url(#${gradientId})`} d={GEMINI_SPARK_PATH} clipPath={`url(#${gradientId}-tip-${i})`}>
              <GeminiContourAnimation key={`${timeline?.key}:${timeline?.durationMs}`} timeline={timeline} />
            </path>
          </g>)}
        </g>
      </g>
    </g>
  );
}

/** OpenCode mark: the pixel-block "o" from the official wordmark
 * (logo-ornate in opencode.ai's bundle), rebuilt on a 24×24 grid — a square
 * ring (block thickness 6) with the signature offset inner block that fills
 * only the lower half of the ring's hole. */
export const OPENCODE_RING_PATH = "M24 0H0V24H24V0ZM18 6H6V18H18V6Z";
// Inner block fills the lower two-thirds of the ring's hole (the wordmark's
// offset-block proportion).
export const OPENCODE_BLOCK_PATH = "M6 10H18V18H6V10Z";

export function opencodeRingGradientStops(
  mood: ClawdMood,
  accent?: string,
  accentDark?: string,
): { from: string; to: string } {
  if (mood === "dead") return { from: "#55585e", to: "#3a3d42" };
  if (mood === "worried") return { from: "#4a5a66", to: "#35434d" };
  // Session rows tint the ring with the per-session palette color; agent tabs
  // keep the brand gradient by not passing an accent.
  if (accent) return { from: accent, to: accentDark || accent };
  if (mood === "sleeping") return { from: "#46655f", to: "#2e4540" };
  // The island's teal (matches the .teal agent-label / subagent tones).
  return { from: "#70d8c8", to: "#2aa896" };
}

export function opencodeBlockFill(mood: ClawdMood): string {
  if (mood === "dead") return "#c8ccd2";
  if (mood === "worried") return "#7cb97c";
  if (mood === "sleeping") return "#dbe7ee";
  // White foreground, matching the ZCode tile's "colored base + white mark"
  // formula — the wordmark's own near-black block vanishes on the dark island.
  return "#ffffff";
}

export function OpencodeOfficialMark({
  mood,
  accent,
  accentDark,
  className,
}: {
  mood: ClawdMood;
  accent?: string;
  accentDark?: string;
  className?: string;
}) {
  // Per-instance gradient id: several mascots with different moods (e.g. a dead
  // header logo next to a live agent tab) must not share one gradient def.
  const gradientId = `opencode-ring-${useId().replace(/[^a-zA-Z0-9]/g, "")}`;
  const { from, to } = opencodeRingGradientStops(mood, accent, accentDark);

  return (
    <g className={className}>
      <defs>
        <linearGradient id={gradientId} x1="0" y1="0" x2="0.35" y2="1">
          <stop offset="0" stopColor={from} />
          <stop offset="1" stopColor={to} />
        </linearGradient>
      </defs>
      {/* Same visual band as the Codex blossom / Cursor cube / ZCode tile / Gemini spark. */}
      <g transform="translate(56 39) scale(3.15) translate(-12 -12)">
        <g data-part="ring" className="opencode-ring"><path fill={`url(#${gradientId})`} fillRule="evenodd" clipRule="evenodd" d={OPENCODE_RING_PATH} /></g>
        <defs><clipPath id={`${gradientId}-aperture`}><rect x="6" y="6" width="12" height="12" /></clipPath></defs>
        <g clipPath={`url(#${gradientId}-aperture)`}>
          <g data-part="block" className="opencode-block"><path fill={opencodeBlockFill(mood)} d={OPENCODE_BLOCK_PATH} /></g>
        </g>
      </g>
    </g>
  );
}

/** Pi mark: the official interlocking block emblem (pi.dev/logo-auto.svg),
 * rebuilt on its 800×800 grid — coral top arm, blue stair-step left, yellow
 * block at the lower right. */
export const PI_LOGO_PATHS = {
  salmon: "M165.29 165.29H517.36V400H400V282.65H165.29Z",
  blue: "M165.29 282.65H282.65V400H400V517.36H282.65V634.72H165.29Z",
  yellow: "M517.36 400H634.72V634.72H517.36Z",
} as const;

export function piMarkColors(
  mood: ClawdMood,
  accent?: string,
  accentDark?: string,
): { salmon: string; blue: string; yellow: string; yellowOpacity: number } {
  if (mood === "dead") return { salmon: "#6a6a6a", blue: "#4e4e4e", yellow: "#7a7a7a", yellowOpacity: 1 };
  if (mood === "worried") return { salmon: "#a97e78", blue: "#4f7485", yellow: "#b39a66", yellowOpacity: 1 };
  if (mood === "sleeping") return { salmon: "#7c5e58", blue: "#2f5567", yellow: "#7b6739", yellowOpacity: 1 };
  if (accent) {
    return { salmon: accent, blue: accentDark || accent, yellow: accent, yellowOpacity: 0.68 };
  }
  return { salmon: "#f09082", blue: "#4d9abf", yellow: "#f1be58", yellowOpacity: 1 };
}

export function PiOfficialMark({
  mood,
  accent,
  accentDark,
  className,
}: {
  mood: ClawdMood;
  accent?: string;
  accentDark?: string;
  className?: string;
}) {
  const colors = piMarkColors(mood, accent, accentDark);
  return (
    <g className={className}>
      {/* Same visual band as the other brand marks (75.6px inside 152×136). */}
      <g data-part="mark">
        <g transform="translate(56 39) scale(0.0945) translate(-400 -400)">
          <path fill={colors.salmon} d={PI_LOGO_PATHS.salmon} />
          <path fill={colors.blue} d={PI_LOGO_PATHS.blue} />
          <path fill={colors.yellow} fillOpacity={colors.yellowOpacity} d={PI_LOGO_PATHS.yellow} />
        </g>
      </g>
    </g>
  );
}

export function CursorOfficialMark({
  palette,
  className,
}: {
  palette: CursorCubePalette;
  className?: string;
}) {
  return (
    <g className={className}>
      <g transform="translate(56 37) scale(0.411) translate(-483.5 -490.5)">
        {/* Keep each non-scaling outline attached to its moving fill. */}
        {(["bottom", "left", "right"] as const).map(part => <g key={part} data-part={`face-${part}`} className={`cursor-mascot-panel cursor-mascot-panel-${part}`}>
          <g className="cursor-mascot-outline" aria-hidden="true"><path d={CURSOR_CUBE_PATHS[part]} fill="none" strokeWidth={2.4} strokeLinejoin="round" strokeLinecap="round" vectorEffect="non-scaling-stroke" /></g>
          <path className={`cursor-mascot-face-${part}`} fill={palette[part]} d={CURSOR_CUBE_PATHS[part]} />
        </g>)}
        <g data-part="pointer" className="cursor-mascot-pointer">
          <g className="cursor-mascot-outline" aria-hidden="true">
            {(["stem", "head"] as const).map(part => <path key={part} d={CURSOR_CUBE_PATHS[part]} fill="none" strokeWidth={2.4} strokeLinejoin="round" strokeLinecap="round" vectorEffect="non-scaling-stroke" />)}
          </g>
          <path className="cursor-mascot-pointer-stem" fill={palette.stem} d={CURSOR_CUBE_PATHS.stem} />
          <path className="cursor-mascot-pointer-head" fill={palette.head} d={CURSOR_CUBE_PATHS.head} />
        </g>
      </g>
    </g>
  );
}

/** DeepSeek Harness mark: the official whale from the DSH web favicon
 * (50×50, path preserved verbatim). It uses the DeepSeek-blue gradient on the
 * dark island; session rows can tint it with their palette color. */
export const DSH_WHALE_PATH =
  "M48.8354 10.0479C48.3232 9.79199 48.1025 10.2798 47.8032 10.5278C47.7007 10.6079 47.6143 10.7119 47.5273 10.8076C46.7793 11.624 45.9048 12.1597 44.7622 12.0957C43.0923 12 41.666 12.5356 40.4058 13.8398C40.1377 12.2319 39.2476 11.272 37.8926 10.6558C37.1836 10.3359 36.4668 10.0156 35.9702 9.31982C35.6235 8.82373 35.5293 8.27197 35.356 7.72754C35.2456 7.3999 35.1353 7.06396 34.7651 7.00781C34.3633 6.94385 34.2056 7.2876 34.0479 7.57568C33.418 8.75195 33.1733 10.0479 33.1973 11.3599C33.2524 14.312 34.4736 16.6641 36.8999 18.3359C37.1758 18.5278 37.2466 18.7197 37.1597 19C36.9946 19.5757 36.7974 20.1357 36.624 20.7119C36.5137 21.0801 36.3486 21.1597 35.9624 21C34.6309 20.4321 33.481 19.5918 32.4644 18.5757C30.7393 16.8721 29.1792 14.9917 27.2334 13.52C26.7764 13.1758 26.3193 12.856 25.8467 12.5518C23.8618 10.584 26.1069 8.96777 26.627 8.77588C27.1704 8.57568 26.8159 7.8877 25.0591 7.896C23.3022 7.90381 21.6953 8.50391 19.647 9.30371C19.3477 9.42383 19.0322 9.51172 18.7095 9.58398C16.8501 9.22363 14.9199 9.14355 12.9033 9.37598C9.10596 9.80762 6.07275 11.6396 3.84326 14.7681C1.16455 18.5278 0.53418 22.7998 1.30664 27.2559C2.11768 31.9521 4.46582 35.8398 8.07373 38.8799C11.8159 42.0322 16.1255 43.5762 21.041 43.2803C24.0269 43.104 27.3516 42.6963 31.1016 39.4561C32.0469 39.936 33.0396 40.1279 34.686 40.272C35.9546 40.3921 37.1758 40.208 38.1211 40.0078C39.6021 39.688 39.4995 38.2881 38.9639 38.0322C34.623 35.9678 35.5762 36.8081 34.71 36.1279C36.9155 33.4639 40.2402 30.6958 41.54 21.728C41.6426 21.0161 41.5557 20.5679 41.54 19.9917C41.5322 19.6396 41.6108 19.5039 42.0049 19.4639C43.0923 19.3359 44.1479 19.0317 45.1167 18.4878C47.9292 16.9199 49.064 14.3438 49.3315 11.2559C49.3711 10.7837 49.3237 10.2959 48.8354 10.0479ZM24.3262 37.8398C20.1196 34.4639 18.0791 33.3521 17.2358 33.3999C16.4482 33.4482 16.5898 34.3682 16.7632 34.9678C16.9443 35.5601 17.1812 35.9683 17.5117 36.4878C17.7402 36.832 17.8979 37.3442 17.2832 37.728C15.9282 38.584 13.5728 37.4399 13.4624 37.3838C10.7207 35.7358 8.42822 33.5601 6.81348 30.584C5.25342 27.7197 4.34766 24.6479 4.19775 21.3677C4.1582 20.5757 4.38672 20.2959 5.15869 20.1519C6.17529 19.96 7.22314 19.9199 8.23926 20.0718C12.5327 20.7119 16.1885 22.6719 19.2529 25.7759C21.002 27.5439 22.3252 29.6558 23.6885 31.7202C25.1377 33.9121 26.6978 36 28.6831 37.7119C29.3843 38.312 29.9434 38.7681 30.479 39.104C28.8643 39.2881 26.1699 39.3281 24.3262 37.8398ZM26.3433 24.6001C26.3433 24.248 26.6191 23.9678 26.9658 23.9678C27.0444 23.9678 27.1152 23.9839 27.1782 24.0078C27.2651 24.04 27.3438 24.0879 27.4067 24.1602C27.5171 24.272 27.5801 24.4321 27.5801 24.6001C27.5801 24.9521 27.3042 25.2319 26.9575 25.2319C26.6108 25.2319 26.3433 24.9521 26.3433 24.6001ZM32.6064 27.8799C32.2046 28.0479 31.8027 28.1919 31.4165 28.208C30.8179 28.2397 30.1641 27.9922 29.8096 27.688C29.2583 27.2158 28.8643 26.9521 28.6987 26.1279C28.6279 25.7759 28.6675 25.2319 28.7305 24.9199C28.8721 24.248 28.7144 23.8159 28.2495 23.4238C27.8716 23.104 27.3911 23.0161 26.8633 23.0161C26.666 23.0161 26.4849 22.9277 26.3511 22.856C26.1304 22.7441 25.9492 22.4639 26.1226 22.1201C26.1777 22.0078 26.4458 21.7358 26.5088 21.688C27.2256 21.272 28.0527 21.4077 28.8169 21.7197C29.5259 22.0161 30.0615 22.5601 30.834 23.3281C31.6216 24.2559 31.7632 24.5117 32.2124 25.208C32.5669 25.752 32.8901 26.312 33.1104 26.9521C33.2446 27.3521 33.0713 27.6802 32.6064 27.8799Z";

export function dshWhaleGradientStops(
  mood: ClawdMood,
  accent?: string,
  accentDark?: string,
): { from: string; to: string } {
  if (mood === "dead") return { from: "#8a8a8a", to: "#666666" };
  if (mood === "worried") return { from: "#7cb97c", to: "#5a8b5a" };
  if (mood === "sleeping") return { from: "#5f6f9a", to: "#3d4a6e" };
  if (accent) return { from: accent, to: accentDark || accent };
  return { from: "#4d6bfe", to: "#2f4bd8" };
}

export function DshOfficialMark({
  mood,
  accent,
  accentDark,
  className,
}: {
  mood: ClawdMood;
  accent?: string;
  accentDark?: string;
  className?: string;
}) {
  const gradientId = `dsh-whale-${useId().replace(/[^a-zA-Z0-9]/g, "")}`;
  const { from, to } = dshWhaleGradientStops(mood, accent, accentDark);
  return (
    <g className={className}>
      <defs>
        <linearGradient id={gradientId} gradientUnits="userSpaceOnUse" x1="0" y1="7" x2="50" y2="43">
          <stop offset="0" stopColor={from} />
          <stop offset="1" stopColor={to} />
        </linearGradient>
      </defs>
      <g transform="translate(56 39) scale(1.5) translate(-25 -25)">
        <g data-part="body" className="dsh-body">
          <path className="mascot-original" fill={`url(#${gradientId})`} d={DSH_WHALE_PATH} fillRule="nonzero" />
          <path className="mascot-articulated" fill={`url(#${gradientId})`} d={DSH_WHALE_PATH} fillRule="nonzero" />
        </g>
      </g>
    </g>
  );
}
