// Artwork backdrop for the expanded island: the full-bleed album art that
// rides the native window resize toward the compact media thumb. Extracted
// verbatim from App.tsx.
import type { CSSProperties } from "react";
import type { NowPlayingTrack } from "../tauri";
import type { ArtworkBackdropOrigin } from "../appTypes";

interface ArtworkBackdropProps {
  nowPlayingTrack: NowPlayingTrack;
  artworkBackdropOrigin: ArtworkBackdropOrigin | null;
  artworkBackdropRevealed: boolean;
  artworkBackdropExitFade: boolean;
  artworkIsDark: boolean;
}

export function ArtworkBackdrop({
  nowPlayingTrack,
  artworkBackdropOrigin,
  artworkBackdropRevealed,
  artworkBackdropExitFade,
  artworkIsDark,
}: ArtworkBackdropProps) {
  return (
    <div
      className={`island-artwork-backdrop${artworkBackdropOrigin ? " has-origin" : ""}${
        artworkBackdropRevealed ? " is-revealed" : ""
      }${artworkBackdropExitFade ? " is-exit-fade" : ""}${artworkIsDark ? " is-dark-art" : ""}`}
      style={
        artworkBackdropOrigin
          ? ({
              // Percent geometry relative to the live window: as the
              // native window shrinks during collapse, the backdrop
              // rides proportionally toward the thumb instead of
              // snapping to pixel coordinates measured pre-expand.
              "--ab-left": `${(artworkBackdropOrigin.x / artworkBackdropOrigin.winW) * 100}%`,
              "--ab-top": `${(artworkBackdropOrigin.y / artworkBackdropOrigin.winH) * 100}%`,
              "--ab-w": `${(artworkBackdropOrigin.w / artworkBackdropOrigin.winW) * 100}%`,
              "--ab-h": `${(artworkBackdropOrigin.h / artworkBackdropOrigin.winH) * 100}%`,
            } as CSSProperties)
          : undefined
      }
      aria-hidden
    >
      <div className="island-artwork-backdrop-scale">
        <div
          className="island-artwork-backdrop-img"
          style={{
            backgroundImage: `url(data:image/jpeg;base64,${nowPlayingTrack.artworkBase64})`,
          }}
        />
        <div
          className="island-artwork-backdrop-ghost"
          style={{
            backgroundImage: `url(data:image/jpeg;base64,${nowPlayingTrack.artworkBase64})`,
          }}
        />
        <div className="island-artwork-backdrop-scrim" />
      </div>
    </div>
  );
}
