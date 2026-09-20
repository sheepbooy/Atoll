import { act, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { LyricsMarquee, lyricsMatchTrack } from "./LyricsMarquee";
import type { LyricPayload, NowPlayingTrack } from "./tauri";

const lines = [
  { timeMs: 0, text: "first line" },
  { timeMs: 60_000, text: "second line" },
];

const payload: LyricPayload = {
  lines,
  currentIndex: 0,
  nextTimeMs: 60_000,
  trackTitle: "Song",
  trackArtist: "Artist",
};

const track: NowPlayingTrack = {
  title: "Song",
  artist: "Artist",
  album: null,
  duration: 200,
  position: 0,
  playing: true,
  artworkBase64: null,
  app: "Music",
};

describe("lyricsMatchTrack", () => {
  it("matches when title and artist are identical", () => {
    expect(lyricsMatchTrack(payload, track)).toBe(true);
  });

  it("rejects lyrics fetched for a different track", () => {
    expect(lyricsMatchTrack(payload, { ...track, title: "Other" })).toBe(false);
    expect(lyricsMatchTrack(payload, { ...track, artist: "Someone" })).toBe(false);
  });

  it("rejects missing lyrics or track", () => {
    expect(lyricsMatchTrack(null, track)).toBe(false);
    expect(lyricsMatchTrack(payload, null)).toBe(false);
  });
});

describe("LyricsMarquee", () => {
  afterEach(() => {
    vi.useRealTimers();
  });

  it("shows the line active at the playback position", () => {
    render(
      <LyricsMarquee
        lines={lines}
        sample={{ position: 70, playing: true, receivedAt: Date.now() }}
      />,
    );
    expect(screen.getByText("second line")).toBeTruthy();
  });

  it("shows the placeholder instead of lines while lyrics are absent", () => {
    render(
      <LyricsMarquee
        lines={[]}
        sample={{ position: 70, playing: true, receivedAt: Date.now() }}
      />,
    );
    expect(screen.getByText("· · ·")).toBeTruthy();
  });

  it("interpolates with the wall clock while playing, crossing the line boundary before the next sample", () => {
    render(
      <LyricsMarquee
        lines={lines}
        sample={{ position: 59.5, playing: true, receivedAt: Date.now() - 1000 }}
      />,
    );
    expect(screen.getByText("second line")).toBeTruthy();
  });

  it("does not interpolate while paused", () => {
    render(
      <LyricsMarquee
        lines={lines}
        sample={{ position: 59.5, playing: false, receivedAt: Date.now() - 1000 }}
      />,
    );
    expect(screen.getByText("first line")).toBeTruthy();
  });

  it("switches lines on the sub-second tick while playing", () => {
    vi.useFakeTimers();
    render(
      <LyricsMarquee
        lines={lines}
        sample={{ position: 59.5, playing: true, receivedAt: Date.now() }}
      />,
    );
    expect(screen.getByText("first line")).toBeTruthy();
    act(() => {
      vi.advanceTimersByTime(500);
    });
    expect(screen.getByText("second line")).toBeTruthy();
  });
});
