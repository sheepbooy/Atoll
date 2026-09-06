import { describe, expect, it } from "vitest";
import {
  OTHER_SENTINEL,
  getOriginalQuestions,
  getPlanModeType,
  isPlanModeCommand,
  parsePlanContent,
  parsePlanQuestions,
  snapshotHasPlanPending,
} from "./planMode";
import type {
  HookHealthSnapshot,
  IslandSnapshot,
  PermissionRequest,
  TokenUsage,
} from "./tauri";

const zeroTokens: TokenUsage = {
  inputTokens: 0,
  outputTokens: 0,
  cacheReadTokens: 0,
  cacheCreationTokens: 0,
};

const hookStatus = {
  installed: true,
  scriptFound: true,
  settingsPath: "/tmp/settings.json",
  scriptPath: "/tmp/atoll-claude-hook.mjs",
};

const hookHealth: HookHealthSnapshot = {
  claude: hookStatus,
  codex: hookStatus,
  cursor: hookStatus,
  zcode: hookStatus,
  gemini: hookStatus,
  opencode: hookStatus,
};

function makeRequest(overrides: Partial<PermissionRequest> = {}): PermissionRequest {
  return {
    id: "req-1",
    agent: "claude",
    session: "session-1",
    command: "AskUserQuestion",
    detail: "",
    cwd: "/tmp",
    requestedAt: new Date().toISOString(),
    status: "pending",
    ...overrides,
  };
}

function makeSnapshot(recent: PermissionRequest[]): IslandSnapshot {
  return {
    online: true,
    pendingCount: recent.filter((request) => request.status === "pending").length,
    archivedCount: 0,
    activeRequest: null,
    recent,
    sessions: [],
    dailyTokens: zeroTokens,
    activeSessionTokens: zeroTokens,
    hookHealth,
  };
}

describe("isPlanModeCommand", () => {
  it("matches bare plan commands", () => {
    expect(isPlanModeCommand("AskUserQuestion")).toBe(true);
    expect(isPlanModeCommand("ExitPlanMode")).toBe(true);
  });

  it("matches plan commands carrying a payload", () => {
    expect(isPlanModeCommand('AskUserQuestion:{"questions":[]}')).toBe(true);
    expect(isPlanModeCommand("ExitPlanMode:{plan:[]}")).toBe(true);
  });

  it("rejects ordinary commands and casing drift", () => {
    expect(isPlanModeCommand("rm -rf /")).toBe(false);
    expect(isPlanModeCommand("askuserquestion")).toBe(false);
    expect(isPlanModeCommand("exitplanmode")).toBe(false);
    expect(isPlanModeCommand("BashAskUserQuestion")).toBe(false);
    expect(isPlanModeCommand("")).toBe(false);
  });
});

describe("snapshotHasPlanPending", () => {
  it("finds a pending plan request among recent entries", () => {
    const snapshot = makeSnapshot([
      makeRequest({ command: "npm install" }),
      makeRequest({ id: "req-2", command: "ExitPlanMode" }),
    ]);
    expect(snapshotHasPlanPending(snapshot)).toBe(true);
  });

  it("ignores resolved plan requests", () => {
    const snapshot = makeSnapshot([
      makeRequest({ command: "ExitPlanMode", status: "approved" }),
    ]);
    expect(snapshotHasPlanPending(snapshot)).toBe(false);
  });

  it("ignores snapshots with only ordinary pending requests", () => {
    const snapshot = makeSnapshot([makeRequest({ command: "npm install" })]);
    expect(snapshotHasPlanPending(snapshot)).toBe(false);
  });
});

describe("getPlanModeType", () => {
  it("classifies question vs exit-plan requests", () => {
    expect(getPlanModeType(makeRequest({ command: "AskUserQuestion" }))).toBe("question");
    expect(getPlanModeType(makeRequest({ command: "ExitPlanMode" }))).toBe("exitPlan");
  });

  it("returns null for ordinary requests", () => {
    expect(getPlanModeType(makeRequest({ command: "npm install" }))).toBeNull();
  });
});

describe("parsePlanContent", () => {
  it("returns a trimmed non-empty plan", () => {
    expect(parsePlanContent({ plan: "  # Step one\n" })).toBe("# Step one");
  });

  it("returns null for blank plans", () => {
    expect(parsePlanContent({ plan: "   " })).toBeNull();
    expect(parsePlanContent({ plan: "" })).toBeNull();
  });

  it("returns null for malformed tool input", () => {
    expect(parsePlanContent(null)).toBeNull();
    expect(parsePlanContent(undefined)).toBeNull();
    expect(parsePlanContent("plan text")).toBeNull();
    expect(parsePlanContent(42)).toBeNull();
    expect(parsePlanContent({})).toBeNull();
    expect(parsePlanContent({ plan: 123 })).toBeNull();
    expect(parsePlanContent({ plan: null })).toBeNull();
  });
});

describe("parsePlanQuestions", () => {
  it("parses well-formed questions", () => {
    const toolInput = {
      questions: [
        {
          question: "Which database?",
          header: "Storage",
          multiSelect: false,
          options: [
            { label: "SQLite", description: "Embedded" },
            { label: "Postgres", description: "Server" },
          ],
        },
      ],
    };
    expect(parsePlanQuestions(toolInput)).toEqual([
      {
        question: "Which database?",
        header: "Storage",
        multiSelect: false,
        options: [
          { label: "SQLite", description: "Embedded" },
          { label: "Postgres", description: "Server" },
        ],
      },
    ]);
  });

  it("coerces multiSelect to a boolean", () => {
    const result = parsePlanQuestions({
      questions: [
        {
          question: "Pick",
          options: [{ label: "A", description: "" }],
          multiSelect: "yes",
        },
      ],
    });
    expect(result[0].multiSelect).toBe(true);
  });

  it("defaults missing header and description to empty strings", () => {
    const result = parsePlanQuestions({
      questions: [{ question: "Pick", options: [{ label: "A" }] }],
    });
    expect(result).toEqual([
      { question: "Pick", header: "", multiSelect: false, options: [{ label: "A", description: "" }] },
    ]);
  });

  it("drops questions without text or without usable options", () => {
    const result = parsePlanQuestions({
      questions: [
        { question: "", options: [{ label: "A", description: "" }] },
        { question: "No options", options: [] },
        { question: "Only blank options", options: [{ label: "", description: "x" }] },
        { question: "Options not an array", options: "nope" },
      ],
    });
    expect(result).toEqual([]);
  });

  it("skips malformed entries and keeps valid neighbours", () => {
    const result = parsePlanQuestions({
      questions: [
        null,
        "question?",
        42,
        { question: "Valid", options: [{ label: "A", description: "" }] },
      ],
    });
    expect(result).toHaveLength(1);
    expect(result[0].question).toBe("Valid");
  });

  it("returns an empty list for malformed tool input", () => {
    expect(parsePlanQuestions(null)).toEqual([]);
    expect(parsePlanQuestions(undefined)).toEqual([]);
    expect(parsePlanQuestions("questions")).toEqual([]);
    expect(parsePlanQuestions({})).toEqual([]);
    expect(parsePlanQuestions({ questions: "nope" })).toEqual([]);
    expect(parsePlanQuestions({ questions: null })).toEqual([]);
  });
});

describe("getOriginalQuestions", () => {
  it("passes the raw questions array through", () => {
    const questions = [{ question: "Q" }];
    expect(getOriginalQuestions({ questions })).toBe(questions);
  });

  it("returns an empty array for malformed input", () => {
    expect(getOriginalQuestions(null)).toEqual([]);
    expect(getOriginalQuestions({})).toEqual([]);
    expect(getOriginalQuestions({ questions: "nope" })).toEqual([]);
  });
});

describe("OTHER_SENTINEL", () => {
  it("keeps its stable sentinel value", () => {
    expect(OTHER_SENTINEL).toBe("__atoll_other__");
  });
});
