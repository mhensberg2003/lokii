// Hooks for the player screen itself: hiding the controls, native fullscreen, keys.

import { useCallback, useEffect, useRef, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { IDLE_AFTER, shortcutFor, type ShortcutAction } from "../lib/player";

/**
 * True after IDLE_AFTER without mouse movement, or when the mouse leaves the window.
 * `hold` keeps the controls visible (paused, a panel open, the mouse on the controls).
 */
export function useIdle(hold: boolean) {
  const [idle, setIdle] = useState(false);
  const timer = useRef<number | undefined>(undefined);

  const poke = useCallback(() => {
    setIdle(false);
    window.clearTimeout(timer.current);
    timer.current = window.setTimeout(() => setIdle(true), IDLE_AFTER);
  }, []);

  useEffect(() => {
    const leave = () => setIdle(true);
    window.addEventListener("pointermove", poke);
    window.addEventListener("pointerdown", poke);
    document.documentElement.addEventListener("mouseleave", leave);
    return () => {
      window.removeEventListener("pointermove", poke);
      window.removeEventListener("pointerdown", poke);
      document.documentElement.removeEventListener("mouseleave", leave);
      window.clearTimeout(timer.current);
    };
  }, [poke]);

  // Count from zero again when the hold ends, so the controls do not vanish at once.
  useEffect(() => {
    if (!hold) poke();
  }, [hold, poke]);

  return { idle: idle && !hold, poke };
}

/** Native window fullscreen. Leaving the player leaves fullscreen. */
export function useFullscreen() {
  const [fullscreen, setState] = useState(false);

  useEffect(() => {
    const win = getCurrentWindow();
    const sync = () => {
      win
        .isFullscreen()
        .then(setState)
        .catch(() => {});
    };
    sync();
    const off = win.onResized(sync);
    return () => {
      off.then((stop) => stop()).catch(() => {});
      win.setFullscreen(false).catch(() => {});
    };
  }, []);

  const setFullscreen = useCallback((on: boolean) => {
    setState(on);
    getCurrentWindow()
      .setFullscreen(on)
      .catch(() => {});
  }, []);

  return { fullscreen, setFullscreen };
}

/** Calls `onAction` for the player keys. Keys with Cmd, Ctrl or Alt stay with the system. */
export function useShortcuts(onAction: (action: ShortcutAction) => void) {
  const handler = useRef(onAction);
  handler.current = onAction;

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.metaKey || event.ctrlKey || event.altKey) return;
      const action = shortcutFor(event.key.length === 1 ? event.key.toLowerCase() : event.key);
      if (!action) return;
      event.preventDefault();
      handler.current(action);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);
}
