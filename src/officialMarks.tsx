import type { ClawdMood } from "./mascotShared";

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

export function codexMarkFill(mood: ClawdMood): string {
  if (mood === "dead") return "#8a8a8a";
  if (mood === "worried") return "#7cb97c";
  if (mood === "sleeping") return "#a8b0b4";
  return "#f4f4f4";
}

export function CodexOfficialMark({ fill, className }: { fill: string; className?: string }) {
  return (
    <g className={className}>
      <g transform="translate(56 38) scale(2.7) translate(-12 -12)">
        <path fill={fill} fillRule="evenodd" clipRule="evenodd" d={CODEX_OFFICIAL_PATH} />
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
      <g transform="translate(56 40) scale(0.345) translate(-482.5 -491.5)">
        <path className="cursor-mascot-face-bottom" fill={palette.bottom} d={CURSOR_CUBE_PATHS.bottom} />
        <path className="cursor-mascot-face-left" fill={palette.left} d={CURSOR_CUBE_PATHS.left} />
        <path className="cursor-mascot-face-right" fill={palette.right} d={CURSOR_CUBE_PATHS.right} />
        <path className="cursor-mascot-pointer-stem" fill={palette.stem} d={CURSOR_CUBE_PATHS.stem} />
        <path className="cursor-mascot-pointer-head" fill={palette.head} d={CURSOR_CUBE_PATHS.head} />
      </g>
    </g>
  );
}
