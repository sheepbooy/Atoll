import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Music, Pause, Play, SkipBack, SkipForward } from "lucide-react";
import type { CSSProperties } from "react";
import type { MediaCommand, NowPlayingTrack } from "./tauri";

interface NowPlayingCardProps {
  track: NowPlayingTrack;
  onCommand: (command: MediaCommand) => void;
}

export function NowPlayingCard({ track, onCommand }: NowPlayingCardProps) {
  const { t } = useTranslation("common");
  const [position, setPosition] = useState(track.position ?? 0);

  // Local progress interpolation: advance position every 1s while playing so
  // the bar moves smoothly between the 2s backend polls.
  useEffect(() => {
    if (!track.playing || track.position == null || track.duration == null) {
      setPosition(track.position ?? 0);
      return;
    }
    setPosition(track.position);
    const interval = window.setInterval(() => {
      setPosition((prev) => {
        if (track.duration != null && prev + 1 >= track.duration) {
          return track.duration;
        }
        return prev + 1;
      });
    }, 1000);
    return () => window.clearInterval(interval);
  }, [track.position, track.playing, track.duration]);

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
