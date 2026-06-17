import { useLayoutEffect, useMemo, useRef } from "react";
import { buildTokenOdometerCells, type TokenOdometerCell } from "./tokenCounterFormat";
import { isPixelGlyphChar, PIXEL_GLYPH_ROWS } from "./pixelFont";

function PixelGlyph({ char, live }: { char: string; live: boolean }) {
  const rows = PIXEL_GLYPH_ROWS[char];
  if (!rows) {
    return <span className="pixel-char pixel-char--other">{char}</span>;
  }

  return (
    <span
      className={`pixel-glyph${live ? " pixel-glyph--live" : ""}`}
      aria-hidden="true"
    >
      {rows.map((row, rowIndex) => (
        <span key={rowIndex} className="pixel-row">
          {[...row].map((bit, colIndex) => (
            <span
              key={colIndex}
              className={`pixel-dot${bit === "1" ? " is-on" : ""}`}
            />
          ))}
        </span>
      ))}
    </span>
  );
}

function PixelCell({
  cell,
  energy,
}: {
  cell: TokenOdometerCell;
  energy: "idle" | "live" | "settle";
}) {
  if (isPixelGlyphChar(cell.char)) {
    const live = energy !== "idle" && (cell.changed || cell.entering);
    return <PixelGlyph char={cell.char} live={live} />;
  }

  return <span className="pixel-char pixel-char--other">{cell.char}</span>;
}

export function PixelDigitDisplay({
  text,
  energy,
}: {
  text: string;
  energy: "idle" | "live" | "settle";
}) {
  const prevTextRef = useRef(text);
  const cells = useMemo(
    () => buildTokenOdometerCells(text, prevTextRef.current),
    [text],
  );

  useLayoutEffect(() => {
    prevTextRef.current = text;
  }, [text]);

  return (
    <span className={`pixel-display pixel-display--${energy}`}>
      {cells.map((cell, index) => (
        <PixelCell
          key={`${index}-${cell.char}-${cell.changed ? cell.prevChar : ""}`}
          cell={cell}
          energy={energy}
        />
      ))}
    </span>
  );
}
