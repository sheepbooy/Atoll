import React from "react";
import ReactDOM from "react-dom/client";
import { App } from "./App";
import { getDemoMode } from "./demoSnapshot";
import "./styles.css";

if ("__TAURI_INTERNALS__" in window) {
  document.documentElement.classList.add("tauri-runtime");
}

if (getDemoMode()) {
  document.documentElement.classList.add("readme-demo");
}

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
