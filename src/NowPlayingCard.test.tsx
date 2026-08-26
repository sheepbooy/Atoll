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
