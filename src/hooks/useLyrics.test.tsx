import { act, renderHook } from "@testing-library/react";
import { beforeEach, expect, it, vi } from "vitest";
import { useLyrics } from "./useLyrics";
import type { LyricPayload } from "../tauri";
const mocks = vi.hoisted(() => ({ get: vi.fn(), changed: null as null | ((p: LyricPayload | null) => void), position: null as null | ((p: { position: number | null; playing: boolean }) => void) }));
vi.mock("../tauri", () => ({ getNowPlaying: vi.fn(async () => null), getLyricsEnabled: vi.fn(async () => true), setLyricsEnabled: vi.fn(async () => false), getCurrentLyrics: mocks.get,
  onLyricsChanged: vi.fn(async cb => { mocks.changed = cb; return () => undefined; }), onLyricsPosition: vi.fn(async cb => { mocks.position = cb; return () => undefined; }) }));
beforeEach(() => mocks.get.mockResolvedValue(null));
const payload: LyricPayload = { lines: [{ timeMs: 0, text: "New lyrics" }], currentIndex: 0, nextTimeMs: null, trackTitle: "New song", trackArtist: "Artist" };
it("does not let a slow initial read overwrite a newer track's lyrics", async () => {
  let resolve!: (p: LyricPayload | null) => void;
  mocks.get.mockReturnValue(new Promise(r => { resolve = r; }));
  const { result } = renderHook(useLyrics);
  await act(async () => {});
  act(() => mocks.changed?.(payload));
  await act(async () => resolve({ ...payload, trackTitle: "Old song" }));
  expect(result.current.lyricsData?.trackTitle).toBe("New song");
});
it("clears lyrics and their local playback clock when disabled or media disappears", async () => {
  const { result } = renderHook(useLyrics);
  await act(async () => {});
  act(() => { mocks.changed?.(payload); mocks.position?.({ position: 12, playing: true }); });
  expect(result.current.playbackPosition?.playing).toBe(true);
  act(() => mocks.changed?.(null));
  expect(result.current.playbackPosition).toBeNull();
  act(() => { mocks.changed?.(payload); mocks.position?.({ position: 20, playing: true }); result.current.handleChangeLyricsEnabled(false); });
  expect(result.current.lyricsData).toBeNull();
  expect(result.current.playbackPosition).toBeNull();
});
it("does not refresh hidden playback UI, and resumes on visibility", async () => {
  const { result, rerender } = renderHook(props => useLyrics(props), { initialProps: { positionEnabled: false } });
  await act(async () => {});
  expect(result.current.playbackPosition).toBeNull();
  rerender({ positionEnabled: true });
  await act(async () => {});
  act(() => mocks.position?.({ position: 30, playing: true }));
  expect(result.current.playbackPosition?.position).toBe(30);
  rerender({ positionEnabled: false });
  expect(result.current.playbackPosition).toBeNull();
  act(() => mocks.position?.({ position: 40, playing: true }));
  expect(result.current.playbackPosition).toBeNull();
});
