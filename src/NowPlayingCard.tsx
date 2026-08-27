import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Music, Pause, Play, SkipBack, SkipForward } from "lucide-react";
import type { CSSProperties } from "react";
import type { MediaCommand, NowPlayingTrack } from "./tauri";

/**
 * A backend playback-position sample. The media monitor emits one every ~1s
 * (`now-playing-position`); `receivedAt` is stamped by the frontend when the
 * event arrives.
 */
export interface LivePositionSample {
  position: number;
  receivedAt: number;
}

interface NowPlayingCardProps {
  track: NowPlayingTrack;
  /**
   * Freshest backend position sample. `track.position` is only a snapshot
   * from the last metadata/play-state change (`now-playing-changed` ignores
   * position), so without these samples the bar free-runs and diverges —
   * permanently after a seek in the source player.
   */
  livePosition?: LivePositionSample | null;
  onCommand: (command: MediaCommand) => void;
}

interface PositionAnchor {
  position: number;
  at: number;
}

export function NowPlayingCard({ track, livePosition, onCommand }: NowPlayingCardProps) {
  const { t } = useTranslation("common");
  const [anchor, setAnchor] = useState<PositionAnchor | null>(() => {
    if (livePosition) {
      return { position: livePosition.position, at: livePosition.receivedAt };
    }
    const pos = track.position;
    return pos != null ? { position: pos, at: Date.now() } : null;
  });
  // Re-render on a sub-second cadence so the wall-clock interpolation below
  // renders smoothly while playing.
  const [, setTick] = useState(0);

  // `track` gets a new object identity on every `now-playing-changed` event;
  // its position snapshot is fresh at that moment (e.g. a new track starting
  // at 0s). Keep whichever sample arrived last so the bar reflects seeks and
  // track changes immediately instead of waiting for the next ~1s poll.
  useEffect(() => {
    const pos = track.position;
    if (pos == null) {
      return;
    }
    const at = Date.now();
    setAnchor((prev) => (!prev || at >= prev.at ? { position: pos, at } : prev));
  }, [track]);

  useEffect(() => {
    if (!livePosition) {
      return;
    }
    const { position, receivedAt: at } = livePosition;
    setAnchor((prev) => (!prev || at >= prev.at ? { position, at } : prev));
  }, [livePosition]);

  useEffect(() => {
    if (!track.playing) {
      return;
    }
    const interval = window.setInterval(() => setTick((n) => n + 1), 250);
    return () => window.clearInterval(interval);
  }, [track.playing]);

  let position = anchor?.position ?? track.position ?? 0;
  if (track.playing && anchor) {
    position += (Date.now() - anchor.at) / 1000;
  }
  if (track.duration != null) {
    position = Math.min(position, track.duration);
  }
  position = Math.max(0, position);

  const progressPct =
    track.duration != null && track.duration > 0
      ? Math.min(100, (position / track.duration) * 100)
      : 0;
  const artworkSrc = track.artworkBase64
    ? `data:image/jpeg;base64,${track.artworkBase64}`
    : null;

  return (
    <div className="now-playing-card">
      <div className="np-artwork">
        {artworkSrc ? (
          <img src={artworkSrc} alt="" className="np-artwork-img" />
        ) : (
          <div className="np-artwork-placeholder">
            <Music size={16} />
          </div>
        )}
      </div>
      <div className="np-info">
        <div className="np-text-row">
          <span className="np-title">{track.title ?? t("media.noMedia")}</span>
          {track.app ? <span className="np-source">{track.app}</span> : null}
        </div>
        <span className="np-artist">{track.artist ?? ""}</span>
        {track.duration != null ? (
          <div className="np-progress">
            <div className="np-progress-bar" style={{ width: `${progressPct}%` }} />
          </div>
        ) : null}
      </div>
      <div className="np-controls">
        <button
          type="button"
          className="np-btn"
          aria-label={t("media.prev")}
          onClick={() => onCommand("prev")}
          data-no-drag
        >
          <SkipBack size={14} />
        </button>
        <button
          type="button"
          className="np-btn np-btn-primary"
          aria-label={track.playing ? t("media.pause") : t("media.play")}
          onClick={() => onCommand("toggle")}
          data-no-drag
        >
          {track.playing ? <Pause size={16} /> : <Play size={16} />}
        </button>
        <button
          type="button"
          className="np-btn"
          aria-label={t("media.next")}
          onClick={() => onCommand("next")}
          data-no-drag
        >
          <SkipForward size={14} />
        </button>
      </div>
    </div>
  );
}
