import { describe, expect, it } from "vitest";
import {
  ACTIVITY_LABELS,
  appStateToActivity,
  canTriggerIdleEasterEgg,
  deriveAppLogoState,
  deriveAtollActivity,
  IDLE_EASTER_EGG_ACTIVITIES,
  isAppStatePose,
  isEasterEggActivity,
} from "./logoStates";

describe("deriveAtollActivity", () => {
  it("maps offline to dead", () => {
    expect(
      deriveAtollActivity({ online: false, pendingCount: 2, sessionCount: 3 }),
    ).toBe("dead");
  });

  it("prioritizes pending over working", () => {
    expect(
      deriveAtollActivity({ online: true, pendingCount: 1, sessionCount: 2 }),
    ).toBe("thinking");
    expect(deriveAppLogoState({ online: true, pendingCount: 1, sessionCount: 2 })).toBe(
      "pending",
    );
  });

  it("maps active sessions to coding when no pending", () => {
    expect(
      deriveAtollActivity({ online: true, pendingCount: 0, sessionCount: 1 }),
    ).toBe("coding");
  });

  it("maps listening idle to idle", () => {
    expect(
      deriveAtollActivity({ online: true, pendingCount: 0, sessionCount: 0 }),
    ).toBe("idle");
  });
});

describe("appStateToActivity", () => {
  it("covers all app states", () => {
    expect(appStateToActivity("idle")).toBe("idle");
    expect(appStateToActivity("pending")).toBe("thinking");
    expect(appStateToActivity("working")).toBe("coding");
    expect(appStateToActivity("offline")).toBe("dead");
  });
});

describe("idle easter eggs", () => {
  it("never overlap app-state poses", () => {
    for (const id of IDLE_EASTER_EGG_ACTIVITIES) {
      expect(isEasterEggActivity(id)).toBe(true);
      expect(isAppStatePose(id)).toBe(false);
      expect(ACTIVITY_LABELS[id]).toBe(id);
    }
  });

  it("only triggers when app state is idle", () => {
    expect(
      canTriggerIdleEasterEgg({ online: true, pendingCount: 0, sessionCount: 0 }),
    ).toBe(true);
    expect(
      canTriggerIdleEasterEgg({ online: true, pendingCount: 1, sessionCount: 0 }),
    ).toBe(false);
    expect(
      canTriggerIdleEasterEgg({ online: true, pendingCount: 0, sessionCount: 2 }),
    ).toBe(false);
    expect(
      canTriggerIdleEasterEgg({ online: false, pendingCount: 0, sessionCount: 0 }),
    ).toBe(false);
  });
});
