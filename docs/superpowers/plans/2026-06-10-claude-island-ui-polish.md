# Claude Island UI Polish Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the jittery two-header Claude approval island with a compact, readable single-header UI that collapses safely, exposes Quit through a menu, and contains no demo behavior.

**Architecture:** Move presentation transitions into a small pure TypeScript state module, then let `App.tsx` coordinate native Tauri resizing with CSS phases. Keep the existing Tauri event bridge, but remove simulation paths, add an explicit quit command, and reduce the expanded native window to 560 by 320 logical pixels.

**Tech Stack:** React 18, TypeScript, Vitest, Testing Library, Vite, Tauri 2, Rust

---

## File Map

- Create `src/islandPresentation.ts`: presentation phases, timing constants, and pure transition helpers.
- Create `src/islandPresentation.test.ts`: transition regression tests.
- Create `src/tauri.test.ts`: browser fallback and quit bridge tests.
- Create `src/App.test.tsx`: user-facing collapse, menu, command rendering, and demo-removal tests.
- Create `src/test/setup.ts`: Testing Library cleanup.
- Modify `package.json` and `vite.config.ts`: add the frontend test runner.
- Modify `src/App.tsx`: single-header layout and transition coordination.
- Modify `src/styles.css`: compact visual hierarchy, code surface, menu, and phased animation.
- Modify `src/tauri.ts`: empty browser fallback, no simulation bridge, and `quitAtoll`.
- Modify `src-tauri/src/lib.rs`: remove demo command/menu, add quit command, and update expanded dimensions.
- Modify `README.md`: remove demo wording.

### Task 1: Add The Presentation State Machine

**Files:**
- Create: `src/islandPresentation.ts`
- Create: `src/islandPresentation.test.ts`
- Create: `src/test/setup.ts`
- Modify: `package.json`
- Modify: `vite.config.ts`

- [ ] **Step 1: Install the frontend test dependencies**

Run:

```bash
npm install --save-dev vitest jsdom @testing-library/react @testing-library/user-event @testing-library/jest-dom
```

Expected: `package.json` and `package-lock.json` contain the new development dependencies.

- [ ] **Step 2: Add the test runner configuration**

Add this script to `package.json`:

```json
"test": "vitest run"
```

Change `vite.config.ts` to:

```ts
import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    host: "127.0.0.1",
    port: 1420,
    strictPort: true,
  },
  test: {
    environment: "jsdom",
    environmentOptions: {
      jsdom: {
        pretendToBeVisual: true,
      },
    },
    setupFiles: "./src/test/setup.ts",
    restoreMocks: true,
  },
});
```

Create `src/test/setup.ts`:

```ts
import "@testing-library/jest-dom/vitest";
import { cleanup } from "@testing-library/react";
import { afterEach } from "vitest";

afterEach(() => cleanup());
```

- [ ] **Step 3: Write the failing presentation tests**

Create `src/islandPresentation.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import {
  COLLAPSE_ANIMATION_MS,
  IDLE_COLLAPSE_DELAY_MS,
  beginCollapse,
  beginExpand,
  finishCollapse,
  finishExpand,
} from "./islandPresentation";

describe("island presentation", () => {
  it("uses deliberate animation and idle delays", () => {
    expect(COLLAPSE_ANIMATION_MS).toBe(420);
    expect(IDLE_COLLAPSE_DELAY_MS).toBe(500);
  });

  it("does not restart an opening or expanded transition", () => {
    expect(beginExpand("compact")).toBe("opening");
    expect(beginExpand("opening")).toBe("opening");
    expect(beginExpand("expanded")).toBe("expanded");
    expect(finishExpand("opening")).toBe("expanded");
  });

  it("can reverse a closing transition when expansion is requested", () => {
    expect(beginExpand("closing")).toBe("opening");
  });

  it("settles closing into compact without duplicating the transition", () => {
    expect(beginCollapse("expanded")).toBe("closing");
    expect(beginCollapse("closing")).toBe("closing");
    expect(finishCollapse("closing")).toBe("compact");
  });
});
```

- [ ] **Step 4: Run the tests and verify RED**

Run:

```bash
npm test -- src/islandPresentation.test.ts
```

Expected: FAIL because `src/islandPresentation.ts` does not exist.

- [ ] **Step 5: Implement the minimal state module**

Create `src/islandPresentation.ts`:

```ts
export type PresentationPhase = "compact" | "opening" | "expanded" | "closing";

export const COLLAPSE_ANIMATION_MS = 420;
export const IDLE_COLLAPSE_DELAY_MS = 500;

export function beginExpand(phase: PresentationPhase): PresentationPhase {
  if (phase === "compact" || phase === "closing") return "opening";
  return phase;
}

export function finishExpand(phase: PresentationPhase): PresentationPhase {
  return phase === "opening" ? "expanded" : phase;
}

export function beginCollapse(phase: PresentationPhase): PresentationPhase {
  if (phase === "opening" || phase === "expanded") return "closing";
  return phase;
}

export function finishCollapse(phase: PresentationPhase): PresentationPhase {
  return phase === "closing" ? "compact" : phase;
}
```

- [ ] **Step 6: Run the presentation tests and full frontend tests**

Run:

```bash
npm test -- src/islandPresentation.test.ts
npm test
```

Expected: PASS with 4 presentation tests.

- [ ] **Step 7: Commit the test harness and state module**

```bash
git add package.json package-lock.json vite.config.ts src/test/setup.ts src/islandPresentation.ts src/islandPresentation.test.ts
git commit -m "test: add island presentation state coverage"
```

### Task 2: Remove Demo Paths And Add Quit To The Tauri Bridge

**Files:**
- Create: `src/tauri.test.ts`
- Modify: `src/tauri.ts`
- Modify: `src-tauri/src/lib.rs`
- Modify: `README.md`

- [ ] **Step 1: Write failing frontend bridge tests**

Create `src/tauri.test.ts`:

```ts
import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  invoke: vi.fn(),
  listen: vi.fn(),
}));

vi.mock("@tauri-apps/api/core", () => ({ invoke: mocks.invoke }));
vi.mock("@tauri-apps/api/event", () => ({ listen: mocks.listen }));

describe("tauri bridge", () => {
  beforeEach(() => {
    vi.resetModules();
    mocks.invoke.mockReset();
    delete (window as Window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__;
  });

  it("starts with an empty browser fallback", async () => {
    const { getSnapshot } = await import("./tauri");
    await expect(getSnapshot()).resolves.toEqual({
      online: true,
      pendingCount: 0,
      activeRequest: null,
      recent: [],
    });
  });

  it("invokes the explicit quit command in Tauri", async () => {
    (window as Window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = {};
    mocks.invoke.mockResolvedValue(undefined);
    const { quitAtoll } = await import("./tauri");

    await quitAtoll();

    expect(mocks.invoke).toHaveBeenCalledWith("quit_atoll");
  });
});
```

- [ ] **Step 2: Run the bridge tests and verify RED**

Run:

```bash
npm test -- src/tauri.test.ts
```

Expected: FAIL because the fallback still contains demo requests and `quitAtoll` is missing.

- [ ] **Step 3: Simplify the frontend bridge**

In `src/tauri.ts`:

- Replace `fallbackRequests` and `localRequests` initialization with:

```ts
let localRequests: PermissionRequest[] = [];
```

- Delete `simulatePermissionRequest`.
- Add:

```ts
export async function quitAtoll() {
  if (!isTauriRuntime) return;
  return invoke<void>("quit_atoll");
}
```

Keep fallback approval resolution so browser interaction tests can still update real fixture requests when tests inject them.

- [ ] **Step 4: Add failing Rust tests for production menu and dimensions**

Add a `core_tests` module near the existing Rust tests:

```rust
#[cfg(test)]
mod core_tests {
    use super::*;

    #[test]
    fn expanded_window_uses_compact_production_dimensions() {
        let size = island_window_logical_size(IslandWindowMode::Expanded);
        assert_eq!(size.width, 560.0);
        assert_eq!(size.height, 320.0);
    }

    #[test]
    fn tray_menu_contains_no_demo_action() {
        assert_eq!(
            tray_menu_entries(),
            [("show", "Show Atoll"), ("quit", "Quit")]
        );
    }
}
```

- [ ] **Step 5: Run the Rust tests and verify RED**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml core_tests
```

Expected: FAIL because the expanded size is still 620 by 360 and `tray_menu_entries` is missing.

- [ ] **Step 6: Remove simulation and implement quit**

In `src-tauri/src/lib.rs`:

- Set:

```rust
const EXPANDED_WINDOW_WIDTH: f64 = 560.0;
const EXPANDED_WINDOW_HEIGHT: f64 = 320.0;
```

- Delete `simulate_permission_request`.
- Add:

```rust
#[tauri::command]
fn quit_atoll(app: AppHandle) {
    app.exit(0);
}
```

- Register only:

```rust
.invoke_handler(tauri::generate_handler![
    get_snapshot,
    resolve_permission_request,
    set_island_presentation,
    quit_atoll
])
```

- Add and use this helper in `build_tray`:

```rust
fn tray_menu_entries() -> [(&'static str, &'static str); 2] {
    [("show", "Show Atoll"), ("quit", "Quit")]
}
```

Construct only the Show and Quit `MenuItem`s, and delete the `"demo"` event arm. Keep tray left-click and **Show Atoll** behavior unchanged.

- [ ] **Step 7: Remove demo wording from the README**

Replace:

```md
- Demo event flow that mirrors Claude/Codex permission requests.
```

with:

```md
- Live permission flow for local coding-agent approval requests.
```

- [ ] **Step 8: Run bridge tests and Rust tests**

Run:

```bash
npm test -- src/tauri.test.ts
cargo test --manifest-path src-tauri/Cargo.toml
```

Expected: all frontend bridge tests and all Rust tests PASS.

- [ ] **Step 9: Commit the production bridge cleanup**

```bash
git add src/tauri.ts src/tauri.test.ts src-tauri/src/lib.rs README.md
git commit -m "feat: remove demo flow and add explicit quit"
```

### Task 3: Rebuild The React Interaction As A Single Header

**Files:**
- Create: `src/App.test.tsx`
- Modify: `src/App.tsx`

- [ ] **Step 1: Write failing interaction tests**

Create `src/App.test.tsx` with mocked Tauri bridge functions and this request fixture:

```tsx
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { App } from "./App";

const request = {
  id: "request-1",
  agent: "claude" as const,
  session: "session-1",
  command: "Bash: npm install --save-dev a-very-long-package-name",
  detail: "Install development dependencies.",
  cwd: "/tmp/project",
  requestedAt: "2026-06-10T08:00:00Z",
  status: "pending" as const,
};

const bridge = vi.hoisted(() => ({
  getSnapshot: vi.fn(),
  onSnapshotChanged: vi.fn(),
  onIslandHoverChanged: vi.fn(),
  onIslandOpenRequested: vi.fn(),
  quitAtoll: vi.fn(),
  resolvePermissionRequest: vi.fn(),
  setIslandPresentation: vi.fn(),
}));

vi.mock("./tauri", () => bridge);
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({ startDragging: vi.fn() }),
}));

describe("App", () => {
  beforeEach(() => {
    bridge.getSnapshot.mockResolvedValue({
      online: true,
      pendingCount: 1,
      activeRequest: request,
      recent: [request],
    });
    bridge.onSnapshotChanged.mockResolvedValue(() => undefined);
    bridge.onIslandHoverChanged.mockResolvedValue(() => undefined);
    bridge.onIslandOpenRequested.mockResolvedValue(() => undefined);
    bridge.setIslandPresentation.mockResolvedValue(undefined);
    bridge.quitAtoll.mockResolvedValue(undefined);
  });

  it("renders the command as code and contains no demo control", async () => {
    render(<App />);
    expect(await screen.findByText(request.command)).toHaveProperty("tagName", "CODE");
    expect(screen.queryByText("Demo")).not.toBeInTheDocument();
    expect(screen.queryByLabelText(/demo/i)).not.toBeInTheDocument();
  });

  it("collapses to the persistent capsule instead of hiding the window", async () => {
    const { container } = render(<App />);
    await waitFor(() => expect(container.querySelector(".is-expanded")).not.toBeNull());

    vi.useFakeTimers();
    fireEvent.click(screen.getByRole("button", { name: "Collapse Atoll" }));
    expect(container.querySelector(".is-closing")).not.toBeNull();

    await vi.advanceTimersByTimeAsync(420);
    expect(bridge.setIslandPresentation).toHaveBeenLastCalledWith("compact");
    expect(container.querySelector(".is-compact")).not.toBeNull();
    vi.useRealTimers();
  });

  it("puts Quit Atoll in the more menu", async () => {
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "More options" }));
    fireEvent.click(screen.getByRole("menuitem", { name: "Quit Atoll" }));
    expect(bridge.quitAtoll).toHaveBeenCalledOnce();
  });

  it("closes the more menu with Escape", async () => {
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "More options" }));
    fireEvent.keyDown(document, { key: "Escape" });
    expect(screen.queryByRole("menu")).not.toBeInTheDocument();
  });

  it("closes the more menu on an outside pointer press", async () => {
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "More options" }));
    fireEvent.pointerDown(document.body);
    expect(screen.queryByRole("menu")).not.toBeInTheDocument();
  });
});
```

- [ ] **Step 2: Run the App tests and verify RED**

Run:

```bash
npm test -- src/App.test.tsx
```

Expected: FAIL because the command is an `h1`, Demo controls exist, Collapse/More actions do not exist, and no quit menu is rendered.

- [ ] **Step 3: Replace the boolean expansion state with presentation phases**

In `src/App.tsx`, import the state helpers and use:

```ts
const [phase, setPhase] = useState<PresentationPhase>("compact");
const phaseRef = useRef<PresentationPhase>("compact");
const transitionTimerRef = useRef<number | null>(null);
const idleTimerRef = useRef<number | null>(null);
const frameRef = useRef<number | null>(null);
const focusedRef = useRef(false);
```

Keep refs and React state synchronized, and cancel stale work with:

```ts
function setPresentationPhase(next: PresentationPhase) {
  phaseRef.current = next;
  setPhase(next);
}

function clearTransitionWork() {
  if (transitionTimerRef.current !== null) {
    window.clearTimeout(transitionTimerRef.current);
    transitionTimerRef.current = null;
  }
  if (frameRef.current !== null) {
    window.cancelAnimationFrame(frameRef.current);
    frameRef.current = null;
  }
}

function clearIdleTimer() {
  if (idleTimerRef.current !== null) {
    window.clearTimeout(idleTimerRef.current);
    idleTimerRef.current = null;
  }
}
```

Implement expansion in this order:

```ts
async function expandIsland() {
  clearTransitionWork();
  const next = beginExpand(phaseRef.current);
  if (next === phaseRef.current) return;
  setPresentationPhase(next);

  try {
    await setIslandPresentation("expanded");
    frameRef.current = window.requestAnimationFrame(() => {
      frameRef.current = null;
      if (phaseRef.current === "opening") {
        setPresentationPhase(finishExpand("opening"));
      }
    });
  } catch {
    setPresentationPhase("compact");
  }
}
```

Implement explicit collapse so it works even with a pending request:

```ts
function collapseIsland() {
  clearTransitionWork();
  setMenuOpen(false);
  const next = beginCollapse(phaseRef.current);
  if (next === phaseRef.current) return;
  setPresentationPhase(next);

  transitionTimerRef.current = window.setTimeout(async () => {
    transitionTimerRef.current = null;
    if (phaseRef.current !== "closing") return;
    try {
      await setIslandPresentation("compact");
      setPresentationPhase(finishCollapse("closing"));
    } catch {
      setPresentationPhase("expanded");
    }
  }, COLLAPSE_ANIMATION_MS);
}
```

Schedule automatic idle collapse with:

```ts
function scheduleIdleCollapse() {
  clearIdleTimer();
  if (
    hoveringRef.current ||
    focusedRef.current ||
    snapshotRef.current.pendingCount > 0
  ) {
    return;
  }

  idleTimerRef.current = window.setTimeout(() => {
    idleTimerRef.current = null;
    if (
      !hoveringRef.current &&
      !focusedRef.current &&
      snapshotRef.current.pendingCount === 0
    ) {
      collapseIsland();
    }
  }, IDLE_COLLAPSE_DELAY_MS);
}
```

Set `focusedRef.current = true` before expanding on focus capture. Set it to
false only when focus leaves the entire island, then call
`scheduleIdleCollapse()`. Remove `onPointerMove`, `hideWindow`, and all calls to
`getCurrentWindow().hide()`.

- [ ] **Step 4: Build one shared header and explicit menu**

Use the phase for root classes. The shell remains compact during `opening` while
the native window grows, then gains `is-expanded` on the next animation frame:

```tsx
const visuallyExpanded = phase === "expanded" || phase === "closing";

<section
  className={`island is-${phase} ${visuallyExpanded ? "is-expanded" : ""}`}
  aria-label="Atoll"
>
```

Render one `island-header` containing the agent indicator, title, compact metadata, pending badge, and expanded-only controls:

```tsx
<div className="header-actions" data-no-drag>
  <button
    className="icon-button"
    type="button"
    onClick={collapseIsland}
    aria-label="Collapse Atoll"
  >
    <ChevronUp size={16} />
  </button>
  <button
    className="icon-button"
    type="button"
    onClick={() => setMenuOpen((open) => !open)}
    aria-label="More options"
    aria-expanded={menuOpen}
  >
    <Ellipsis size={17} />
  </button>
  {menuOpen ? (
    <div className="more-menu" role="menu">
      <button type="button" role="menuitem" onClick={handleQuit}>
        <Power size={15} />
        Quit Atoll
      </button>
    </div>
  ) : null}
</div>
```

Implement quit and menu dismissal with:

```ts
async function handleQuit() {
  setMenuOpen(false);
  await quitAtoll().catch(() => undefined);
}

useEffect(() => {
  if (!menuOpen) return;

  function closeOnPointerDown(event: PointerEvent) {
    if (!menuRef.current?.contains(event.target as Node)) {
      setMenuOpen(false);
    }
  }

  function closeOnEscape(event: KeyboardEvent) {
    if (event.key === "Escape") setMenuOpen(false);
  }

  document.addEventListener("pointerdown", closeOnPointerDown);
  document.addEventListener("keydown", closeOnEscape);
  return () => {
    document.removeEventListener("pointerdown", closeOnPointerDown);
    document.removeEventListener("keydown", closeOnEscape);
  };
}, [menuOpen]);
```

Attach `menuRef` to the header-actions wrapper so presses on the trigger and menu
do not count as outside presses.

- [ ] **Step 5: Make the approval content command-first and remove Demo UI**

Delete `createDemoRequest`, all `Play` imports, the queue Demo chip, and the empty-state demo button. Render the command as:

```tsx
<code className="command-block">{request.command}</code>
```

Keep detail and working directory below it. Keep approval buttons outside the scrollable request body. `IdleView` accepts no props and contains only the icon, “All clear”, and explanatory copy.

- [ ] **Step 6: Run the App tests and full frontend suite**

Run:

```bash
npm test -- src/App.test.tsx
npm test
```

Expected: all App tests and all frontend tests PASS.

- [ ] **Step 7: Commit the interaction rewrite**

```bash
git add src/App.tsx src/App.test.tsx src/test/setup.ts package.json package-lock.json
git commit -m "feat: rebuild island interaction and quit menu"
```

### Task 4: Polish The Visual Design And Animation

**Files:**
- Modify: `src/styles.css`

- [ ] **Step 1: Replace two-header styles with phase-based shell styles**

Use these timing and sizing rules as the baseline:

```css
.island {
  width: 132px;
  height: 28px;
  border-radius: 0 0 15px 15px;
  background:
    linear-gradient(180deg, rgba(22, 23, 22, 0.99), rgba(8, 9, 9, 0.98));
  box-shadow:
    inset 0 -1px 0 rgba(255, 255, 255, 0.08),
    0 8px 24px rgba(0, 0, 0, 0.24);
  overflow: hidden;
  transition:
    width 420ms cubic-bezier(0.22, 1, 0.36, 1),
    height 420ms cubic-bezier(0.22, 1, 0.36, 1),
    border-radius 420ms cubic-bezier(0.22, 1, 0.36, 1),
    box-shadow 300ms ease;
}

.island.is-expanded {
  width: min(560px, 100vw);
  height: min(320px, 100vh);
  border-radius: 0 0 24px 24px;
}

.island.is-opening .island-panel {
  opacity: 0;
  transform: translateY(-8px);
}

.island.is-expanded:not(.is-closing) .island-panel {
  opacity: 1;
  transform: translateY(0);
}

.island.is-closing .island-panel {
  opacity: 0;
  transform: translateY(-8px);
  pointer-events: none;
}
```

Ensure only `.island-header` remains; remove `.panel-header`, `.window-actions`, `.compact-action`, and duplicate close styles.

- [ ] **Step 2: Apply the compact command hierarchy**

Use:

```css
.command-block {
  display: block;
  padding: 10px 12px;
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 12px;
  color: #f5f1e8;
  background: rgba(255, 255, 255, 0.055);
  font-family: "SFMono-Regular", Consolas, "Liberation Mono", monospace;
  font-size: 12px;
  line-height: 1.55;
  overflow-wrap: anywhere;
  white-space: pre-wrap;
}

.request-copy p {
  color: #aaa69b;
  font-size: 12px;
  line-height: 1.45;
}

.cwd-line {
  font-size: 10px;
}
```

Keep the request content scrollable and decision buttons fixed. Keep the header title at 13px or less and supporting metadata at 11px.

- [ ] **Step 3: Style the more menu and reduced motion**

Add a positioned menu with clear destructive text, then:

```css
@media (prefers-reduced-motion: reduce) {
  .island,
  .island-panel,
  .island-header,
  .agent-dot,
  .compact-meta {
    transition-duration: 1ms !important;
  }
}
```

- [ ] **Step 4: Run automated frontend verification**

Run:

```bash
npm test
npm run build
```

Expected: all tests PASS and Vite production build exits 0.

- [ ] **Step 5: Commit the visual polish**

```bash
git add src/styles.css
git commit -m "style: polish Claude approval island"
```

### Task 5: Verify The Complete Desktop Flow

**Files:**
- Modify only if verification reveals a defect.

- [ ] **Step 1: Run all automated checks**

Run:

```bash
npm test
npm run build
cargo test --manifest-path src-tauri/Cargo.toml
node scripts/atoll-claude-hook.test.mjs
```

Expected: every command exits 0 with no failed tests.

- [ ] **Step 2: Start the desktop application**

Run:

```bash
npm run tauri dev
```

Keep the process running for manual verification.

- [ ] **Step 3: Verify the compact and expanded UI**

Using the desktop app:

1. Confirm startup shows the 132 by 28 top-center capsule.
2. Hover the capsule and confirm expansion is smooth and takes about 420ms.
3. Move out and confirm the idle island waits 500ms before collapsing.
4. Repeat hover entry and exit quickly; confirm no flash, jump, or stale resize.
5. Click **Collapse Atoll** and confirm the capsule remains visible.
6. Confirm only one header is visible and no duplicate close button remains.

- [ ] **Step 4: Verify live request readability and decisions**

Send a Claude hook request containing a long shell command. Confirm:

1. The island stays expanded while the request is pending.
2. The complete command is visible in a 12px monospace code surface and wraps.
3. Detail and cwd remain subordinate.
4. Approve and Deny stay visible below scrolling content.
5. Resolving the request returns to the idle state without a layout jump.

- [ ] **Step 5: Verify menu, tray, and production cleanup**

Confirm:

1. No Demo button, queue chip, tray item, seeded request, or README demo claim remains.
2. **More options** opens a menu containing only **Quit Atoll**.
3. Escape and outside click close the menu.
4. Tray **Show Atoll** expands a running instance.
5. With macOS Reduce Motion enabled, shell movement is effectively disabled.
6. Restore the normal motion setting, then confirm **Quit Atoll** terminates the
   process.

- [ ] **Step 6: Inspect the final diff and status**

Run:

```bash
git diff 1c197af --check
git status --short
```

Expected: no whitespace errors and no unintended files.
