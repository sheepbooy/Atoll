import type { LyricLine, LyricPayload, NowPlayingTrack } from "./tauri";

interface LyricsMarqueeProps {
  lines: LyricLine[];
  /** Playback position in seconds (from backend, updated every 1s). */
  position: number | null;
  playing: boolean;
}

/**
 * True when the lyrics payload was fetched for the track currently playing.
 * Lyrics are fetched asynchronously on track change; until the new payload
 * arrives, the old track's lines must not be rendered against the new
 * track's position (they would show unrelated lines mid-song).
 */
export function lyricsMatchTrack(
  lyrics: LyricPayload | null,
  track: NowPlayingTrack | null,
): boolean {
  if (lyrics == null || track == null) {
    return false;
  }
  return lyrics.trackTitle === track.title && lyrics.trackArtist === track.artist;
}

/**
 * Renders the current lyric line with a vertical fade-in transition.
 *
 * Sync strategy: the backend polls position every 1s via the MediaRemote
 * adapter (`get --now`) and emits it. We derive the current line directly
 * from that position — no local interpolation. The 1s cadence is tight
 * enough for lyric sync (most lines last several seconds), and avoiding
 * interpolation eliminates drift (the local clock running faster/slower
 * than the actual player).
 */
export function LyricsMarquee({ lines, position, playing }: LyricsMarqueeProps) {
  const pos = position ?? 0;
  const currentIndex = lineIndexAt(lines, pos);
  const line = lines[currentIndex];
  const text = line?.text.trim() ?? "";

  // The wrapper must stay mounted even when the current line is empty
  // (intro/interlude): it occupies the dedicated 140px middle grid column,
  // so unmounting it would shift the metrics column inward and expose a
  // blank strip of island background to its right.
  return (
    <span className="lyrics-marquee" aria-hidden="true">
      {text ? (
        <span key={currentIndex} className="lyrics-marquee__line">
          {line.text}
        </span>
      ) : (
        <span className="lyrics-marquee__gap">· · ·</span>
      )}
    </span>
  );
}

/** Binary search for the line active at `positionSec`. */
function lineIndexAt(lines: LyricLine[], positionSec: number): number {
  if (lines.length === 0) return 0;
  const posMs = positionSec * 1000;
  let lo = 0;
  let hi = lines.length;
  while (lo < hi) {
    const mid = (lo + hi) >> 1;
    if (lines[mid].timeMs <= posMs) {
      lo = mid + 1;
    } else {
      hi = mid;
    }
  }
  return Math.max(0, lo - 1);
}
