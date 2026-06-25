import React from "react";
import ReactDOM from "react-dom/client";
import { App } from "./App";
import { BrandExportPage, getBrandExportMode } from "./BrandExport";
import { CursorMascotPreviewPage, getCursorPreviewMode } from "./CursorMascotPreview";
import { getDemoMode } from "./demoSnapshot";
import "./styles.css";

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
      <App />
    </React.StrictMode>,
  );
}
