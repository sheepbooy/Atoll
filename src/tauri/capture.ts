import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { isTauriRuntime } from "./runtime";
export async function onCaptureCollapseRequested(callback: () => void) {
  if (!isTauriRuntime()) {
    return () => undefined;
  }

  return listen<void>("capture-collapse", () => callback());
}

export async function onCaptureOpenHooksRequested(callback: () => void) {
  if (!isTauriRuntime()) {
    return () => undefined;
  }

  return listen<void>("capture-open-hooks", () => callback());
}

export async function onCaptureScreenshotRequested(callback: () => void | Promise<void>) {
  if (!isTauriRuntime()) {
    return () => undefined;
  }

  return listen<void>("capture-screenshot-requested", () => callback());
}

export async function captureProvideScreenshot(pngBase64: string) {
  if (!isTauriRuntime()) {
    return;
  }

  await invoke("capture_provide_screenshot", { pngBase64 });
}
