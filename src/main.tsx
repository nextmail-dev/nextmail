import React from "react";
import ReactDOM from "react-dom/client";
import "@fontsource/roboto/latin-400.css";
import "@fontsource/roboto/latin-500.css";
import "@fontsource/roboto/latin-700.css";
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
