# Claude Island UI Polish Design

## Goal

Polish the existing Claude permission approval island so that it feels stable,
compact, and easy to read. Remove development-only demo controls and make the
difference between collapsing the island and quitting Atoll explicit.

## Current Problems

- Expanding and collapsing feels abrupt and can visibly stutter because the
  native Tauri window size and React content transition change at different
  times.
- The expanded view gives the command headline treatment, which makes long
  shell commands hard to scan.
- Demo requests and controls remain visible in the production interaction.
- Two close buttons perform the same window-hide action.
- Hiding the window removes the only visible way to reopen it, even though the
  tray can still show it.

## Interaction Model

Atoll has two normal presentation states:

- **Compact:** A persistent top-center capsule remains visible and provides the
  primary way to expand Atoll.
- **Expanded:** A single header and approval panel are visible.

The expanded header has two actions:

- **Collapse:** Returns Atoll to its compact capsule. It does not hide or quit
  the application.
- **More:** Opens a small menu containing **Quit Atoll**. Quit is the only UI
  action that terminates the application.

The existing tray remains a secondary recovery path and keeps a **Show Atoll**
action. Closing the expanded panel never makes the application disappear.

## Layout And Visual Hierarchy

The compact and expanded presentations share one top header instead of
rendering a compact header plus a second panel header.

In the expanded approval view:

- Agent, listening state, request age, and pending count are supporting
  metadata.
- The command is displayed in a dedicated code surface using a 12px monospace
  font.
- Long commands wrap inside the code surface. The request area scrolls when its
  content exceeds the available height.
- Request detail and working directory use quieter colors and smaller type than
  the command.
- Deny and Approve remain the strongest actions and stay fixed below the
  scrollable request content.

The expanded window uses 560 by 320 logical pixels. Long commands scroll within
the request area while both decision buttons remain fixed and visible.

## Animation

Presentation changes use an explicit transition phase rather than treating
`expanded` as a single immediate boolean.

### Expand

1. Mark the transition as opening and cancel any pending collapse.
2. Resize the native Tauri window to the expanded bounds.
3. On the next rendered frame, animate the island shell and content into view.
4. Complete the visual transition in approximately 420ms.

### Collapse

1. Close the more menu and mark the transition as closing.
2. Fade and translate the expanded content out while the native window remains
   expanded.
3. After the visual transition completes, resize the native window to compact
   bounds.
4. Return to the stable compact state.

Repeated hover, focus, tray, and request events must cancel stale timers and
must not start a second transition toward the state already being entered.
Pending approval requests keep the island expanded. An idle island collapses
500ms after pointer and focus both leave it.

Users who prefer reduced motion receive near-instant opacity changes without
the shell movement.

## Demo Removal

Remove production demo behavior from all layers:

- Empty-state demo button.
- Queue-strip Demo button.
- Tray **Create Demo Request** item.
- Frontend `simulatePermissionRequest` bridge.
- Rust `simulate_permission_request` command.
- Browser fallback seed requests and demo request creation.
- README language that describes the app as a demo flow.

When no real request exists, the empty state remains informational only.

## Application Exit

Add a Tauri command dedicated to quitting Atoll and expose it through the
frontend bridge. The **Quit Atoll** menu item calls this command.

The menu must:

- Open only from the expanded header.
- Close on outside click, Escape, collapse, or successful quit request.
- Use clear text rather than a second ambiguous close icon.

## Error Handling

- Presentation command failures leave the UI in the last usable stable state
  and clear transition timers.
- Approval failures continue to release the busy state so the user can retry.
- A failed quit invocation closes the menu but does not hide the island.

## Verification

- Run the TypeScript and Vite production build.
- Run Rust tests for the Tauri core and Claude hook bridge.
- Manually verify compact-to-expanded and expanded-to-compact transitions under
  repeated hover and focus changes.
- Verify long shell commands wrap and remain readable.
- Verify no Demo control or seeded request appears.
- Verify Collapse always leaves the compact capsule visible.
- Verify **Quit Atoll** terminates the process and the tray **Show Atoll** action
  still expands a running application.
- Verify reduced-motion behavior.

## Scope

This work changes the island presentation and removes development-only demo
features. It does not change Claude hook payload mapping, approval semantics,
request queue ordering, or add configuration persistence.
