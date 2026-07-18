import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./app/i18n";
import { applyDesktopPlatform } from "./app/platform";
import "./styles/globals.css";

applyDesktopPlatform();

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);

const startupShell = document.getElementById("startup-shell");
if (startupShell) {
  window.requestAnimationFrame(() => {
    window.requestAnimationFrame(() => startupShell.remove());
  });
}
