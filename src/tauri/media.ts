import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { isTauriRuntime } from "./runtime";
export interface NowPlayingTrack {
  title: string | null;
  artist: string | null;
  album: string | null;
  duration: number | null;
  position: number | null;
  playing: boolean;
  artworkBase64: string | null;
  app: string | null;
}

export type MediaCommand = "play" | "pause" | "toggle" | "next" | "prev";

export async function getNowPlaying(): Promise<NowPlayingTrack | null> {
  if (!isTauriRuntime()) {
    return null;
  }
  return invoke<NowPlayingTrack | null>("get_now_playing");
}

export async function sendMediaCommand(command: MediaCommand): Promise<boolean> {
  if (!isTauriRuntime()) {
    return false;
  }
  return invoke<boolean>("send_media_command", { command });
}

export async function getMediaCardEnabled(): Promise<boolean> {
  if (!isTauriRuntime()) {
    return true;
  }
  return invoke<boolean>("get_media_card_enabled");
}

export async function setMediaCardEnabled(enabled: boolean): Promise<boolean> {
  if (!isTauriRuntime()) {
    return enabled;
  }
  return invoke<boolean>("set_media_card_enabled", { enabled });
}

export async function getArtworkBackdropEnabled(): Promise<boolean> {
  if (!isTauriRuntime()) {
    return false;
  }
  return invoke<boolean>("get_artwork_backdrop_enabled");
}

export async function setArtworkBackdropEnabled(enabled: boolean): Promise<boolean> {
  if (!isTauriRuntime()) {
    return enabled;
  }
  return invoke<boolean>("set_artwork_backdrop_enabled", { enabled });
}

export interface LyricLine {
  timeMs: number;
  text: string;
}

export interface LyricPayload {
  lines: LyricLine[];
  currentIndex: number;
  nextTimeMs: number | null;
  trackTitle: string | null;
  trackArtist: string | null;
}

export async function getLyricsEnabled(): Promise<boolean> {
  if (!isTauriRuntime()) {
    return false;
  }
  return invoke<boolean>("get_lyrics_enabled");
}

export async function setLyricsEnabled(enabled: boolean): Promise<boolean> {
  if (!isTauriRuntime()) {
    return enabled;
  }
  return invoke<boolean>("set_lyrics_enabled", { enabled });
}

export async function getCurrentLyrics(): Promise<LyricPayload | null> {
  if (!isTauriRuntime()) {
    return null;
  }
  return invoke<LyricPayload | null>("get_current_lyrics");
}

export async function onLyricsChanged(
  callback: (payload: LyricPayload | null) => void,
) {
  if (!isTauriRuntime()) {
    return () => undefined;
  }
  return listen<LyricPayload | null>("lyrics-changed", (event) =>
    callback(event.payload),
  );
}

export interface LyricsPosition {
  /** Null when the player omitted elapsedTime; consumers should skip. */
  position: number | null;
  playing: boolean;
}

export async function onLyricsPosition(
  callback: (pos: LyricsPosition) => void,
) {
  if (!isTauriRuntime()) {
    return () => undefined;
  }
  return listen<LyricsPosition>("now-playing-position", (event) =>
    callback(event.payload),
  );
}

export async function onNowPlayingChanged(
  callback: (track: NowPlayingTrack | null) => void,
) {
  if (!isTauriRuntime()) {
    return () => undefined;
  }
  return listen<NowPlayingTrack | null>("now-playing-changed", (event) =>
    callback(event.payload),
  );
}
