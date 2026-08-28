import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ClipboardHistoryView } from "./ClipboardHistoryView";
import type { ClipboardEntry } from "./tauri";

vi.mock("./tauri", async (importOriginal) => {
  const actual = await importOriginal<typeof import("./tauri")>();
  return {
    ...actual,
    getClipboardEntryThumbnail: vi.fn().mockResolvedValue(null),
  };
});

function makeEntry(overrides: Partial<ClipboardEntry> = {}): ClipboardEntry {
  return {
    id: "1",
    kind: "text",
    content: "hello",
    preview: "hello",
    copiedAt: Math.floor(Date.now() / 1000),
    favorited: false,
    ...overrides,
  };
}

describe("ClipboardHistoryView", () => {
  it("lists recent history and hides expired favorites from the history tab", () => {
    const expiredFavorite = makeEntry({
      id: "old",
      content: "ancient",
      preview: "ancient",
      copiedAt: Math.floor(Date.now() / 1000) - 25 * 60 * 60,
      favorited: true,
    });
    render(
      <ClipboardHistoryView
        entries={[makeEntry({ id: "fresh", preview: "fresh copy" }), expiredFavorite]}
        enabled
        onCopy={vi.fn()}
        onClear={vi.fn()}
        onToggleFavorite={vi.fn()}
      />,
    );

    expect(screen.getByText("fresh copy")).toBeInTheDocument();
    expect(screen.queryByText("ancient")).not.toBeInTheDocument();
    expect(screen.getByRole("tab", { name: "History" })).toHaveAttribute(
      "aria-selected",
      "true",
    );
  });

  it("shows only favorited items on the favorites tab", () => {
    render(
      <ClipboardHistoryView
        entries={[
          makeEntry({ id: "a", preview: "plain" }),
          makeEntry({ id: "b", preview: "kept", favorited: true }),
        ]}
        enabled
        onCopy={vi.fn()}
        onClear={vi.fn()}
        onToggleFavorite={vi.fn()}
      />,
    );

    fireEvent.click(screen.getByRole("tab", { name: "Favorites" }));
    expect(screen.getByText("kept")).toBeInTheDocument();
    expect(screen.queryByText("plain")).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /Clear all/i })).not.toBeInTheDocument();
  });

  it("stars an item without copying it", () => {
    const onCopy = vi.fn();
    const onToggleFavorite = vi.fn();
    render(
      <ClipboardHistoryView
        entries={[makeEntry({ preview: "star me" })]}
        enabled
        onCopy={onCopy}
        onClear={vi.fn()}
        onToggleFavorite={onToggleFavorite}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Favorite" }));
    expect(onToggleFavorite).toHaveBeenCalledWith("1");
    expect(onCopy).not.toHaveBeenCalled();
  });

  it("still lets favorites be opened when history recording is off", () => {
    render(
      <ClipboardHistoryView
        entries={[makeEntry({ preview: "pinned", favorited: true })]}
        enabled={false}
        onCopy={vi.fn()}
        onClear={vi.fn()}
        onToggleFavorite={vi.fn()}
      />,
    );

    expect(screen.getByText("Enable clipboard history in settings")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("tab", { name: "Favorites" }));
    expect(screen.getByText("pinned")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Unfavorite" })).toBeInTheDocument();
  });

  it("calls onClear from the history tab", () => {
    const onClear = vi.fn();
    render(
      <ClipboardHistoryView
        entries={[makeEntry({ preview: "wipe me" })]}
        enabled
        onCopy={vi.fn()}
        onClear={onClear}
        onToggleFavorite={vi.fn()}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: /Clear all/i }));
    expect(onClear).toHaveBeenCalledOnce();
  });
});
