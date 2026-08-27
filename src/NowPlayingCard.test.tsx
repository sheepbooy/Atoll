import { render, screen, fireEvent } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { NowPlayingCard } from "./NowPlayingCard";
import type { NowPlayingTrack } from "./tauri";

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string) => {
      const map: Record<string, string> = {
        "media.noMedia": "Nothing playing",
        "media.play": "Play",
        "media.pause": "Pause",
        "media.next": "Next",
        "media.prev": "Previous",
      };
      return map[key] ?? key;
    },
  }),
}));

const baseTrack: NowPlayingTrack = {
  title: null,
  artist: null,
  album: null,
  duration: null,
  position: null,
  playing: false,
  artworkBase64: null,
  app: null,
};

describe("NowPlayingCard", () => {
  it("renders track title and artist", () => {
    render(
      <NowPlayingCard
        track={{ ...baseTrack, title: "Test Song", artist: "Test Artist", app: "Music" }}
        onCommand={() => {}}
      />,
    );
    expect(screen.getByText("Test Song")).toBeTruthy();
    expect(screen.getByText("Test Artist")).toBeTruthy();
    expect(screen.getByText("Music")).toBeTruthy();
  });

  it("calls onCommand with toggle when play/pause button clicked", () => {
    const onCommand = vi.fn();
    render(
      <NowPlayingCard track={{ ...baseTrack, playing: false }} onCommand={onCommand} />,
    );
    fireEvent.click(screen.getByRole("button", { name: "Play" }));
    expect(onCommand).toHaveBeenCalledWith("toggle");
  });

  it("shows pause icon when playing", () => {
    render(
      <NowPlayingCard track={{ ...baseTrack, playing: true }} onCommand={() => {}} />,
    );
    expect(screen.getByRole("button", { name: "Pause" })).toBeTruthy();
  });

  it("renders artwork image when artworkBase64 is set", () => {
    const { container } = render(
      <NowPlayingCard
        track={{ ...baseTrack, artworkBase64: "aGVsbG8=", title: "With Art" }}
        onCommand={() => {}}
      />,
    );
    expect(container.querySelector(".np-artwork-img")).toBeTruthy();
  });

  it("renders placeholder when no artwork", () => {
    const { container } = render(
      <NowPlayingCard track={{ ...baseTrack, title: "No Art" }} onCommand={() => {}} />,
    );
    expect(container.querySelector(".np-artwork-placeholder")).toBeTruthy();
  });
});

describe("NowPlayingCard progress sync", () => {
  const playingTrack: NowPlayingTrack = {
    ...baseTrack,
    title: "Song",
    duration: 200,
    position: 10,
    playing: true,
  };

  function barWidthPct(container: HTMLElement): number {
    const bar = container.querySelector<HTMLElement>(".np-progress-bar");
    expect(bar).not.toBeNull();
    return parseFloat((bar as HTMLElement).style.width);
  }

  it("resyncs the bar to backend position samples, including backward seeks", () => {
    const now = vi.spyOn(Date, "now").mockReturnValue(60_000);
    const { container, rerender } = render(
      <NowPlayingCard track={playingTrack} onCommand={() => {}} />,
    );
    // No live sample yet: fall back to the track's position snapshot.
    expect(barWidthPct(container)).toBeCloseTo(5);

    rerender(
      <NowPlayingCard
        track={playingTrack}
        livePosition={{ position: 100, receivedAt: 60_000 }}
        onCommand={() => {}}
      />,
    );
    expect(barWidthPct(container)).toBeCloseTo(50);

    // A backward seek in the source player must be reflected, not ignored.
    now.mockReturnValue(61_000);
    rerender(
      <NowPlayingCard
        track={playingTrack}
        livePosition={{ position: 30, receivedAt: 61_000 }}
        onCommand={() => {}}
      />,
    );
    expect(barWidthPct(container)).toBeCloseTo(15);
  });

  it("advances along the wall clock between backend samples while playing", () => {
    const now = vi.spyOn(Date, "now").mockReturnValue(60_000);
    const track: NowPlayingTrack = { ...playingTrack, position: 40 };
    const { container, rerender } = render(
      <NowPlayingCard
        track={track}
        livePosition={{ position: 40, receivedAt: 60_000 }}
        onCommand={() => {}}
      />,
    );
    expect(barWidthPct(container)).toBeCloseTo(20);

    // Two seconds pass with no new sample: interpolation fills the gap.
    now.mockReturnValue(62_000);
    rerender(
      <NowPlayingCard
        track={track}
        livePosition={{ position: 40, receivedAt: 60_000 }}
        onCommand={() => {}}
      />,
    );
    expect(barWidthPct(container)).toBeCloseTo(21);
  });

  it("freezes the bar while paused", () => {
    const now = vi.spyOn(Date, "now").mockReturnValue(60_000);
    const pausedTrack: NowPlayingTrack = { ...playingTrack, playing: false, position: 100 };
    const { container, rerender } = render(
      <NowPlayingCard
        track={pausedTrack}
        livePosition={{ position: 100, receivedAt: 60_000 }}
        onCommand={() => {}}
      />,
    );
    expect(barWidthPct(container)).toBeCloseTo(50);

    now.mockReturnValue(65_000);
    rerender(
      <NowPlayingCard
        track={pausedTrack}
        livePosition={{ position: 100, receivedAt: 60_000 }}
        onCommand={() => {}}
      />,
    );
    expect(barWidthPct(container)).toBeCloseTo(50);
  });

  it("clamps the bar at the track duration", () => {
    const now = vi.spyOn(Date, "now").mockReturnValue(60_000);
    const { container, rerender } = render(
      <NowPlayingCard
        track={playingTrack}
        livePosition={{ position: 199, receivedAt: 60_000 }}
        onCommand={() => {}}
      />,
    );
    // Interpolated position would overshoot to 201s at t=62s.
    now.mockReturnValue(62_000);
    rerender(
      <NowPlayingCard
        track={playingTrack}
        livePosition={{ position: 199, receivedAt: 60_000 }}
        onCommand={() => {}}
      />,
    );
    expect(barWidthPct(container)).toBe(100);
  });

  it("prefers a fresh track snapshot over a stale live sample on track change", () => {
    const now = vi.spyOn(Date, "now").mockReturnValue(60_000);
    const { container, rerender } = render(
      <NowPlayingCard
        track={playingTrack}
        livePosition={{ position: 180, receivedAt: 60_000 }}
        onCommand={() => {}}
      />,
    );
    expect(barWidthPct(container)).toBeCloseTo(90);

    // The track changed at t=61s; the live sample is still from t=60s.
    now.mockReturnValue(61_000);
    const nextTrack: NowPlayingTrack = { ...playingTrack, title: "Next Song", position: 0 };
    rerender(
      <NowPlayingCard
        track={nextTrack}
        livePosition={{ position: 180, receivedAt: 60_000 }}
        onCommand={() => {}}
      />,
    );
    expect(barWidthPct(container)).toBe(0);
  });
});
