import { vi } from "vitest";

/** A live media query, so tests exercise changes while animations are running. */
export function mockMotionPreference(initial = false) {
  const listeners = new Set<() => void>();
  const query = {
    matches: initial,
    media: "(prefers-reduced-motion: reduce)",
    addEventListener: (_name: string, listener: () => void) => listeners.add(listener),
    removeEventListener: (_name: string, listener: () => void) => listeners.delete(listener),
  };
  vi.stubGlobal("matchMedia", vi.fn(() => query));
  return (reduced: boolean) => {
    query.matches = reduced;
    for (const listener of [...listeners]) listener();
  };
}
