import React from "react";
import ReactDOM from "react-dom/client";
import { I18nextProvider } from "react-i18next";
import { App } from "./App";
import { AppErrorBoundary } from "./AppErrorBoundary";
import { BrandExportPage, getBrandExportMode } from "./BrandExport";
import { CursorMascotPreviewPage, getCursorPreviewMode } from "./CursorMascotPreview";
import { getDemoMode } from "./demoSnapshot";
import { getNotchMetrics } from "./tauri/island";
import { applyWindowMetrics } from "./islandLayout";
import i18n from "./i18n";
import { injectAnimationTimingVars } from "./animationTiming";
import "./styles.css";

// TS/CSS 共享的动效时长先于首次 render 注入，CSS 侧 var() 才有值。
injectAnimationTimingVars();

if ("__TAURI_INTERNALS__" in window) {
  document.documentElement.classList.add("tauri-runtime");
}

const demoMode = getDemoMode();
if (demoMode === "gif") {
  document.documentElement.classList.add("tauri-runtime", "gif-capture");
} else if (demoMode) {
  document.documentElement.classList.add("readme-demo");
}

const root = ReactDOM.createRoot(document.getElementById("root")!);

// 刘海 metrics 先于首帧水合：原生窗口在 setup 时已按摄像头模组定型，
// 第一帧就必须带 has-notch 适配，否则刘海机上会闪一帧未适配布局。
// 150ms 兜底：IPC 异常时不能拖住启动，交给 hook 内的常规水合路径。
const NOTCH_HYDRATION_TIMEOUT_MS = 150;

async function bootstrap() {
  if ("__TAURI_INTERNALS__" in window) {
    try {
      const notch = await Promise.race([
        getNotchMetrics(),
        new Promise<null>((resolve) =>
          setTimeout(() => resolve(null), NOTCH_HYDRATION_TIMEOUT_MS),
        ),
      ]);
      if (notch) {
        applyWindowMetrics(notch);
      }
    } catch {
      // 常规水合路径（useIslandPresentation）会再取一次并落到同样的 CSS 变量。
    }
  }

  if (getBrandExportMode()) {
    root.render(
      <React.StrictMode>
        <BrandExportPage />
      </React.StrictMode>,
    );
  } else if (getCursorPreviewMode()) {
    root.render(
      <React.StrictMode>
        <CursorMascotPreviewPage />
      </React.StrictMode>,
    );
  } else {
    root.render(
      <React.StrictMode>
        <I18nextProvider i18n={i18n}>
          <AppErrorBoundary>
            <App />
          </AppErrorBoundary>
        </I18nextProvider>
      </React.StrictMode>,
    );
  }
}

void bootstrap();
