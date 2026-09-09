import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import i18n from "./app/i18n";
import { applyDesktopPlatform } from "./app/platform";
import { setupGlobalErrorReporting } from "./app/errorReporting";
import "./styles/globals.css";
import { initializeDemoSession, isDemoMode } from "./app/demo/session";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { api } from "./app/api";
import { Button } from "./components/ui/button";
import { AppShell, Stack } from "./components/ui/layout";
import { Heading, Text } from "./components/ui/typography";

applyDesktopPlatform();
setupGlobalErrorReporting();
const root = ReactDOM.createRoot(document.getElementById("root") as HTMLElement);

async function start() {
  await initializeDemoSession();
  if (isDemoMode() && new URLSearchParams(window.location.search).has("window")) {
    await getCurrentWindow().destroy();
    return;
  }
  root.render(
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
  if (isDemoMode()) {
    window.requestAnimationFrame(() => window.requestAnimationFrame(() => {
      void getCurrentWindow().show().then(() => getCurrentWindow().setFocus());
    }));
  }
}
void start().catch(() => {
  // Fail closed: never mount real data if process-mode discovery fails.
  document.getElementById("startup-shell")?.remove();
  root.render(
    <AppShell className="grid place-items-center bg-card p-8">
      <Stack className="items-center text-center" gap="md">
        <Heading>NextMail</Heading>
        <Text>{i18n.t("common.unexpectedError")}</Text>
        <div className="flex gap-2">
          <Button variant="secondary" onClick={() => void api.quitApp()}>{i18n.t("desktop.quit")}</Button>
          <Button onClick={() => window.location.reload()}>{i18n.t("common.retry")}</Button>
        </div>
      </Stack>
    </AppShell>,
  );
  void getCurrentWindow().show();
});
