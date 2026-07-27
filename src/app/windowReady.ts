import { getCurrentWindow } from "@tauri-apps/api/window";
import { useEffect, useRef } from "react";

import { reportCaughtError } from "./errorReporting";

export function useRevealWindowWhenReady(ready: boolean, focus = true) {
  const revealed = useRef(false);

  useEffect(() => {
    if (!ready || revealed.current || !("__TAURI_INTERNALS__" in globalThis)) return;
    revealed.current = true;
    const appWindow = getCurrentWindow();
    void appWindow.show()
      .then(() => focus ? appWindow.setFocus() : undefined)
      .catch((error) => {
        revealed.current = false;
        reportCaughtError("window.reveal", error);
      });
  }, [focus, ready]);
}
