import { useEffect, useState } from "react";
import type { LyricLine, LyricPayload, NowPlayingTrack } from "./tauri";
import type { PlaybackPositionSample } from "./hooks/useLyrics";

interface LyricsMarqueeProps {
  lines: LyricLine[];
  /** Freshest backend position sample; `receivedAt` anchors interpolation. */
  sample: PlaybackPositionSample | null;
}

/**
 * Seconds subtracted from the interpolated position before picking the line.
 * The backend samples the player, then a perl/IPC round-trip precedes
 * `receivedAt`, so interpolation alone still trails the audio by that latency;
 * the 400ms fade-up also needs a head start to peak on the vocal. The lead
 * compensates for both — raise it if lines feel late, lower it if early.
 */
const LYRICS_SYNC_LEAD_S = 0.15;

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
 * Shared media samples anchor the local clock. Between samples, wake at the
 * next lyric boundary rather than repainting four times per second.
 */
export function LyricsMarquee({ lines, sample }: LyricsMarqueeProps) {
  const [, setTick] = useState(0);

  let pos = sample?.position ?? 0;
  if (sample?.playing) {
    pos += (Date.now() - sample.receivedAt) / 1000 + LYRICS_SYNC_LEAD_S;
  }
  const currentIndex = lineIndexAt(lines, pos);
  const nextTime = lines[currentIndex + 1]?.timeMs;
  useEffect(() => {
    if (!sample?.playing || nextTime === undefined) return;
    const nowPosition = sample.position * 1000 + Date.now() - sample.receivedAt + LYRICS_SYNC_LEAD_S * 1000;
    const timer = window.setTimeout(() => setTick(n => n + 1), Math.max(1, Math.min(2_147_483_647, nextTime - nowPosition)));
    return () => window.clearTimeout(timer);
  }, [sample, nextTime]);
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
