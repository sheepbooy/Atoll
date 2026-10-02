import { useCallback, useEffect, useState } from "react";
import {
  NowPlayingTrack,
  getNowPlaying,
  getMediaCardEnabled,
  getArtworkBackdropEnabled,
  onNowPlayingChanged,
  setMediaCardEnabled,
  setArtworkBackdropEnabled,
} from "../tauri";
import { manageAsyncUnlisten } from "../asyncUnlisten";
import { sampleArtworkIsDark } from "../artwork";

export function useNowPlaying() {
  const [nowPlayingTrack, setNowPlayingTrack] = useState<NowPlayingTrack | null>(null);
  const [mediaCardEnabled, setMediaCardEnabledState] = useState(true);
  const [artworkBackdropEnabled, setArtworkBackdropEnabledState] = useState(false);
  const [artworkIsDark, setArtworkIsDark] = useState(false);

  useEffect(() => {
    let received = false;
    let cancelled = false;
    getNowPlaying().then(track => { if (!received && !cancelled) setNowPlayingTrack(track); }).catch(() => undefined);
    getMediaCardEnabled()
      .then(setMediaCardEnabledState)
      .catch(() => undefined);
    getArtworkBackdropEnabled()
      .then(setArtworkBackdropEnabledState)
      .catch(() => undefined);
    const unsubscribe = manageAsyncUnlisten(
      onNowPlayingChanged((track) => {
        received = true;
        setNowPlayingTrack(track);
      }),
    );
    return () => {
      cancelled = true;
      unsubscribe();
    };
  }, []);

  // Re-test backdrop luminance whenever the artwork changes.
  useEffect(() => {
    const base64 = nowPlayingTrack?.artworkBase64;
    if (!base64) return;
    let cancelled = false;
    sampleArtworkIsDark(base64).then((dark) => {
      if (!cancelled) setArtworkIsDark(dark);
    });
    return () => {
      cancelled = true;
    };
  }, [nowPlayingTrack?.artworkBase64]);

  const handleChangeMediaCardEnabled = useCallback((enabled: boolean) => {
    setMediaCardEnabledState(enabled);
    setMediaCardEnabled(enabled).catch(() => undefined);
  }, []);

  const handleChangeArtworkBackdropEnabled = useCallback((enabled: boolean) => {
    setArtworkBackdropEnabledState(enabled);
    setArtworkBackdropEnabled(enabled).catch(() => undefined);
  }, []);

  return {
    nowPlayingTrack,
    mediaCardEnabled,
    artworkBackdropEnabled,
    artworkIsDark,
    handleChangeMediaCardEnabled,
    handleChangeArtworkBackdropEnabled,
  };
}
