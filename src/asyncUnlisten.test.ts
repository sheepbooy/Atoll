import { describe, expect, it, vi } from "vitest";
import { manageAsyncUnlisten } from "./asyncUnlisten";

describe("manageAsyncUnlisten", () => {
  it("runs cleanup immediately if registration resolves after disposal", async () => {
    const cleanup = vi.fn();
    let resolveRegistration!: (cleanup: () => void) => void;
    const registration = new Promise<() => void>((resolve) => {
      resolveRegistration = resolve;
    });

    const dispose = manageAsyncUnlisten(registration);
    dispose();
    resolveRegistration(cleanup);
    await registration;

    expect(cleanup).toHaveBeenCalledOnce();
  });

  it("runs cleanup on disposal after registration resolves", async () => {
    const cleanup = vi.fn();
    const dispose = manageAsyncUnlisten(Promise.resolve(cleanup));
    await Promise.resolve();

    dispose();

    expect(cleanup).toHaveBeenCalledOnce();
  });
});
