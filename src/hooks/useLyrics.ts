import { useCallback, useEffect, useState } from "react";
import {
  getLyricsEnabled,
  setLyricsEnabled,
  onLyricsChanged,
  onLyricsPosition,
  getCurrentLyrics,
  getNowPlaying,
  type LyricPayload,
} from "../tauri";
import { manageAsyncUnlisten } from "../asyncUnlisten";

/**
 * A backend playback-position sample. The media monitor emits one every ~1s;
 * `receivedAt` anchors wall-clock interpolation between samples.
 */
export interface PlaybackPositionSample {
  position: number;
  playing: boolean;
  receivedAt: number;
}

export function useLyrics({ positionEnabled = true }: { positionEnabled?: boolean } = {}) {
  const [lyricsData, setLyricsData] = useState<LyricPayload | null>(null);
  const [playbackPosition, setPlaybackPosition] =
    useState<PlaybackPositionSample | null>(null);
  const [lyricsEnabled, setLyricsEnabledState] = useState(false);

  useEffect(() => {
    let received = false;
    let cancelled = false;
    getLyricsEnabled()
      .then(setLyricsEnabledState)
      .catch(() => undefined);
    getCurrentLyrics()
      .then(payload => { if (!cancelled && !received) setLyricsData(payload); })
      .catch(() => undefined);
    const unsubscribeChanged = manageAsyncUnlisten(
      onLyricsChanged((payload) => {
        received = true;
        setLyricsData(payload);
        if (!payload) setPlaybackPosition(null);
      }),
    );
    return () => {
      cancelled = true;
      unsubscribeChanged();
    };
  }, []);

  useEffect(() => {
    if (!positionEnabled) { setPlaybackPosition(null); return; }
    let received = false;
    let cancelled = false;
    getNowPlaying().then(track => {
      if (!cancelled && !received && track?.position != null) setPlaybackPosition({ position: track.position, playing: track.playing, receivedAt: Date.now() });
    }).catch(() => undefined);
    const unsubscribePosition = manageAsyncUnlisten(
      onLyricsPosition(({ position, playing }) => {
        if (cancelled || position == null) {
          return;
        }
        received = true;
        setPlaybackPosition({ position, playing, receivedAt: Date.now() });
      }),
    );
    return () => {
      cancelled = true;
      unsubscribePosition();
    };
  }, [positionEnabled]);

  const handleChangeLyricsEnabled = useCallback((enabled: boolean) => {
    setLyricsEnabledState(enabled);
    setLyricsEnabled(enabled).catch(() => undefined);
    if (!enabled) {
      setLyricsData(null);
      setPlaybackPosition(null);
    }
  }, []);

  return { lyricsData, playbackPosition, lyricsEnabled, handleChangeLyricsEnabled };
}
