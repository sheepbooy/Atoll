import { useEffect, useRef, useState } from "react";
import { isTextEntryActive } from "../imeHelpers";
import { manageAsyncUnlisten } from "../asyncUnlisten";
import {
  onReminderCreateRequested,
  type ReminderDue,
  type ReminderSnapshot,
} from "../tauri/reminders";
import { onIslandHoverChanged, deactivateAtoll } from "../tauri";
import type { PanelView } from "../appTypes";
import { pendingReminderOccurrences } from "../reminderFormat";
export function useReminderPresentation(options: {
  due: ReminderDue | null;
  clearDue: () => void;
  snapshot: ReminderSnapshot;
  pendingCount: number;
  dragging: boolean;
  phase: string;
  panelViewRef: { current: PanelView };
  setPanelView: (view: PanelView) => void;
  expand: () => void;
  resize: () => void;
  collapse: () => void;
  clearIdle: () => void;
  holdRef: { current: boolean };
}) {
  const live = useRef(options);
  live.current = options;
  const [focusToken, setFocusToken] = useState(0);
  const [alertOpen, setAlertOpen] = useState(false);
  const alertRef = useRef(false);
  const held = useRef(false);
  const wasExpanded = useRef(false);
  const autoTimer = useRef<ReturnType<typeof setTimeout>>();
  const hovered = useRef(false);
  const shortcut = useRef(false);
  const queue = useRef(new Set<string>());
  const interacted = useRef(false);
  function open(shortcutOpen = false) {
    const o = live.current;
    close(false);
    shortcut.current = shortcutOpen;
    o.panelViewRef.current = { kind: "reminders" };
    o.setPanelView({ kind: "reminders" });
    o.resize();
    o.expand();
    setFocusToken((x) => x + 1);
    o.clearIdle();
  }
  function close(restore = true) {
    clearTimeout(autoTimer.current);
    alertRef.current = false;
    live.current.holdRef.current = false;
    setAlertOpen(false);
    held.current = false;
    if (
      restore &&
      !wasExpanded.current &&
      !live.current.pendingCount &&
      !live.current.dragging
    )
      live.current.collapse();
  }
  function hold() {
    if (!alertRef.current) return;
    interacted.current = true;
    held.current = true;
    clearTimeout(autoTimer.current);
    live.current.clearIdle();
  }
  useEffect(() => {
    const create = manageAsyncUnlisten(
      onReminderCreateRequested(() => open(true)),
    );
    const hover = manageAsyncUnlisten(
      onIslandHoverChanged((e) => {
        hovered.current = e.hovering;
        if (e.hovering) hold();
      }),
    );
    const focus = () => {
      queueMicrotask(attempt);
    };
    document.addEventListener("focusout", focus);
    return () => {
      create();
      hover();
      document.removeEventListener("focusout", focus);
      clearTimeout(autoTimer.current);
      live.current.holdRef.current = false;
    };
  }, []);
  function attempt() {
    const o = live.current;
    if (
      !queue.current.size ||
      o.pendingCount ||
      o.dragging ||
      isTextEntryActive() ||
      o.phase === "opening" ||
      o.phase === "closing"
    )
      return;
    const pending = pendingReminderOccurrences(o.snapshot, Date.now() / 1000);
    if (!pending.some((item) => queue.current.has(item.id))) return;
    if (!alertRef.current) {
      wasExpanded.current = o.phase === "expanded";
      interacted.current = false;
      alertRef.current = true;
      setAlertOpen(true);
      o.holdRef.current = true;
      o.clearIdle();
      o.expand();
      if (hovered.current) held.current = true;
    }
    queue.current.clear();
    clearTimeout(autoTimer.current);
    if (!held.current)
      autoTimer.current = setTimeout(() => close(!interacted.current), 8000);
  }
  useEffect(() => {
    if (!options.due) return;
    if (options.due.fallbackSound) {
      try {
        const audio = new AudioContext();
        const oscillator = audio.createOscillator();
        const gain = audio.createGain();
        oscillator.frequency.value = 660;
        gain.gain.value = 0.06;
        oscillator.connect(gain);
        gain.connect(audio.destination);
        void audio.resume().then(() => {
          oscillator.start();
          oscillator.stop(audio.currentTime + 0.18);
        });
        oscillator.onended = () => {
          void audio.close();
        };
      } catch {
        /* Reminder remains visible when audio is unavailable. */
      }
    }
    if (options.due.noticeMode !== "notify")
      for (const item of options.due.occurrences) queue.current.add(item.id);
    options.clearDue();
    attempt();
  }, [options.due]);
  useEffect(() => {
    if ((options.pendingCount || options.dragging) && alertRef.current) {
      for (const item of pendingReminderOccurrences(
        options.snapshot,
        Date.now() / 1000,
      ))
        queue.current.add(item.id);
      close(false);
    }
    if (options.phase === "closing" && alertRef.current) close(false);
    if (
      alertRef.current &&
      !pendingReminderOccurrences(options.snapshot, Date.now() / 1000).length
    )
      close();
    attempt();
  }, [options.pendingCount, options.dragging, options.phase, options.snapshot]);
  function created() {
    if (!shortcut.current) return;
    shortcut.current = false;
    (document.activeElement as HTMLElement | null)?.blur();
    live.current.collapse();
    void deactivateAtoll();
  }
  return {
    openReminders: () => open(false),
    focusToken,
    reminderAlertOpen: alertOpen,
    dismissReminderAlert: () => close(),
    holdReminderAlert: hold,
    reminderCreated: created,
  };
}
