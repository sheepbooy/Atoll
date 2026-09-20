import { useCallback, useEffect, useState } from "react";
import {
  getLyricsEnabled,
  setLyricsEnabled,
  onLyricsChanged,
  onLyricsPosition,
  getCurrentLyrics,
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

export function useLyrics() {
  const [lyricsData, setLyricsData] = useState<LyricPayload | null>(null);
  const [playbackPosition, setPlaybackPosition] =
    useState<PlaybackPositionSample | null>(null);
  const [lyricsEnabled, setLyricsEnabledState] = useState(false);

  useEffect(() => {
    getLyricsEnabled()
      .then(setLyricsEnabledState)
      .catch(() => undefined);
    getCurrentLyrics()
      .then(setLyricsData)
      .catch(() => undefined);
    const unsubscribeChanged = manageAsyncUnlisten(
      onLyricsChanged((payload) => {
        setLyricsData(payload);
      }),
    );
    const unsubscribePosition = manageAsyncUnlisten(
      onLyricsPosition(({ position, playing }) => {
        if (position == null) {
          return;
        }
        setPlaybackPosition({ position, playing, receivedAt: Date.now() });
      }),
    );
    return () => {
      unsubscribeChanged();
      unsubscribePosition();
    };
  }, []);

  const handleChangeLyricsEnabled = useCallback((enabled: boolean) => {
    setLyricsEnabledState(enabled);
    setLyricsEnabled(enabled).catch(() => undefined);
    if (!enabled) {
      setLyricsData(null);
    }
  }, []);

  return { lyricsData, playbackPosition, lyricsEnabled, handleChangeLyricsEnabled };
}
