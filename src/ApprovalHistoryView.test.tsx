import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { ApprovalHistoryView } from "./ApprovalHistoryView";
import type { ApprovalHistoryEntry } from "./tauri";
import {
  clearApprovalHistory,
  exportApprovalHistory,
  getApprovalHistory,
  revealPath,
} from "./tauri";

vi.mock("./tauri", async (importOriginal) => {
  const actual = await importOriginal<typeof import("./tauri")>();
  return {
    ...actual,
    getApprovalHistory: vi.fn(),
    exportApprovalHistory: vi.fn(),
    clearApprovalHistory: vi.fn(),
    revealPath: vi.fn(),
  };
});

const mockGet = vi.mocked(getApprovalHistory);
const mockExport = vi.mocked(exportApprovalHistory);
const mockClear = vi.mocked(clearApprovalHistory);
const mockReveal = vi.mocked(revealPath);

function makeEntry(overrides: Partial<ApprovalHistoryEntry> = {}): ApprovalHistoryEntry {
  return {
    id: "1",
    agent: "claude",
    sessionId: "sess-alpha",
    command: "Bash: ls",
    detail: "Bash: ls Approved from Atoll",
    cwd: "/work/alpha",
    requestedAt: Math.floor(Date.now() / 1000),
    decidedAt: Math.floor(Date.now() / 1000),
    status: "approved",
    host: "claudeCli",
    ...overrides,
  };
}

describe("ApprovalHistoryView", () => {
  beforeEach(() => {
    mockGet.mockResolvedValue({ items: [], total: 0 });
    mockExport.mockResolvedValue(null);
    mockClear.mockResolvedValue(undefined);
    mockReveal.mockResolvedValue(undefined);
  });

  it("lists history rows with outcome badge, agent and project", async () => {
    mockGet.mockResolvedValue({
      items: [
        makeEntry({ id: "a", command: "Bash: ls", status: "approved" }),
        makeEntry({
          id: "b",
          command: "Bash: rm -rf node_modules",
          cwd: "/work/beta",
          agent: "codex",
          status: "denied",
        }),
      ],
      total: 2,
    });
    render(<ApprovalHistoryView />);

    expect(await screen.findByText("Bash: ls")).toBeInTheDocument();
    expect(screen.getByText("Bash: rm -rf node_modules")).toBeInTheDocument();
    // Outcome badges plus the matching filter pills.
    expect(screen.getAllByText("Approved").length).toBeGreaterThanOrEqual(2);
    expect(screen.getAllByText("Denied").length).toBeGreaterThanOrEqual(2);
    expect(screen.getAllByText("Claude").length).toBeGreaterThan(0);
    expect(screen.getAllByText("Codex").length).toBeGreaterThan(0);
    expect(screen.getAllByText(/alpha/).length).toBeGreaterThan(0);
  });

  it("debounces the search box before querying", async () => {
    mockGet.mockImplementation(async (query) => ({
      items: query?.search ? [makeEntry({ id: "hit" })] : [],
      total: query?.search ? 1 : 0,
    }));
    render(<ApprovalHistoryView />);

    await waitFor(() => expect(mockGet).toHaveBeenCalled());
    const callsAfterMount = mockGet.mock.calls.length;

    fireEvent.change(screen.getByRole("textbox"), {
      target: { value: "cargo" },
    });
    // Not debounced yet: the in-flight query still has no search term.
    expect(mockGet).toHaveBeenLastCalledWith(expect.not.objectContaining({ search: "cargo" }));

    await waitFor(() =>
      expect(mockGet).toHaveBeenLastCalledWith(
        expect.objectContaining({ search: "cargo" }),
      ),
    );
    expect(mockGet.mock.calls.length).toBeGreaterThan(callsAfterMount);
  });

  it("applies agent and outcome filter pills", async () => {
    render(<ApprovalHistoryView />);
    await waitFor(() => expect(mockGet).toHaveBeenCalled());

    fireEvent.click(screen.getByRole("button", { name: "Codex" }));
    await waitFor(() =>
      expect(mockGet).toHaveBeenLastCalledWith(
        expect.objectContaining({ agent: "codex" }),
      ),
    );

    fireEvent.click(screen.getByRole("button", { name: "Expired" }));
    await waitFor(() =>
      expect(mockGet).toHaveBeenLastCalledWith(
        expect.objectContaining({ agent: "codex", status: "expired" }),
      ),
    );
  });

  it("expands a row to show details and filters by session chip", async () => {
    mockGet.mockResolvedValue({
      items: [makeEntry({ id: "a", host: "claudeCli" })],
      total: 1,
    });
    render(<ApprovalHistoryView />);
    fireEvent.click(await screen.findByText("Bash: ls"));

    expect(screen.getByText("Project")).toBeInTheDocument();
    expect(screen.getByText("/work/alpha")).toBeInTheDocument();
    expect(screen.getByText("Host")).toBeInTheDocument();
    expect(screen.getByText("sess-alpha")).toBeInTheDocument();

    fireEvent.click(screen.getByText("sess-alpha"));
    await waitFor(() =>
      expect(mockGet).toHaveBeenLastCalledWith(
        expect.objectContaining({ sessionId: "sess-alpha" }),
      ),
    );
    expect(screen.getByText("Session: sess-alpha")).toBeInTheDocument();
  });

  it("shows the right empty state", async () => {
    mockGet.mockResolvedValue({ items: [], total: 0 });
    const { unmount } = render(<ApprovalHistoryView />);
    expect(await screen.findByText("Approval requests will be recorded here")).toBeInTheDocument();
    unmount();

    mockGet.mockResolvedValue({ items: [], total: 0 });
    render(<ApprovalHistoryView />);
    await waitFor(() => expect(mockGet).toHaveBeenCalled());
    fireEvent.click(screen.getByRole("button", { name: "Claude" }));
    expect(await screen.findByText("No matching approval history")).toBeInTheDocument();
  });

  it("exports JSON and reveals the exported file", async () => {
    mockGet.mockResolvedValue({ items: [makeEntry()], total: 1 });
    mockExport.mockResolvedValue("/tmp/atoll-history-1.json");
    render(<ApprovalHistoryView />);
    await screen.findByText("Bash: ls");

    fireEvent.click(screen.getByRole("button", { name: "Export JSON" }));
    expect(await screen.findByText(/Exported to \/tmp\/atoll-history-1\.json/)).toBeInTheDocument();
    expect(mockExport).toHaveBeenCalledWith(
      expect.anything(),
      "json",
    );

    fireEvent.click(screen.getByRole("button", { name: "Show in folder" }));
    await waitFor(() => expect(mockReveal).toHaveBeenCalledWith("/tmp/atoll-history-1.json"));
  });

  it("shows an error toast when export fails", async () => {
    mockGet.mockResolvedValue({ items: [makeEntry()], total: 1 });
    mockExport.mockRejectedValue(new Error("disk full"));
    render(<ApprovalHistoryView />);
    await screen.findByText("Bash: ls");

    fireEvent.click(screen.getByRole("button", { name: "Export CSV" }));
    expect(await screen.findByText("Export failed")).toBeInTheDocument();
    expect(mockExport).toHaveBeenCalledWith(expect.anything(), "csv");
  });

  it("loads more pages and appends rows", async () => {
    const page1 = [makeEntry({ id: "1" }), makeEntry({ id: "2" })];
    const page2 = [makeEntry({ id: "3" }), makeEntry({ id: "4" })];
    mockGet.mockImplementation(async (query) =>
      query?.offset
        ? { items: page2, total: 4 }
        : { items: page1, total: 4 },
    );
    render(<ApprovalHistoryView />);
    expect((await screen.findAllByText("Bash: ls")).length).toBe(2);

    fireEvent.click(screen.getByRole("button", { name: "Load more" }));
    await waitFor(() => expect(screen.getAllByText("Bash: ls")).toHaveLength(4));
    expect(mockGet).toHaveBeenLastCalledWith(
      expect.objectContaining({ offset: 2 }),
    );
  });

  it("clears the history", async () => {
    mockGet.mockResolvedValue({ items: [makeEntry()], total: 1 });
    render(<ApprovalHistoryView />);
    await screen.findByText("Bash: ls");

    fireEvent.click(screen.getByRole("button", { name: "Clear history" }));
    await waitFor(() => expect(mockClear).toHaveBeenCalled());
    expect(await screen.findByText("Approval history cleared")).toBeInTheDocument();
  });
});
