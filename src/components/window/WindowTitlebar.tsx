import { getCurrentWindow } from "@tauri-apps/api/window";
import { Maximize2, Minus, X } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";

import { cn } from "@/lib/utils";
import { detectDesktopPlatform } from "@/app/platform";

export type WindowKind = "main" | "composer" | "settings" | "accounts" | "message-preview" | "raw-message" | "definition" | "update";

export function WindowTitlebar({ kind }: { kind: WindowKind }) {
  const { t } = useTranslation();
  const platform = useMemo(() => detectDesktopPlatform(), []);
  const isMac = platform === "macos";
  const isWindows = platform === "windows";
  const appWindow = getCurrentWindow();
  const title = windowTitle(kind, t);
  const [active, setActive] = useState(() => document.hasFocus());

  useEffect(() => {
    const handleFocus = () => setActive(true);
    const handleBlur = () => setActive(false);
    window.addEventListener("focus", handleFocus);
    window.addEventListener("blur", handleBlur);
    return () => {
      window.removeEventListener("focus", handleFocus);
      window.removeEventListener("blur", handleBlur);
    };
  }, []);

  return (
    <header
      className={cn(
        "window-titlebar fixed inset-x-0 top-0 flex h-[var(--titlebar-height)] select-none items-center",
        isMac && "window-titlebar--mac",
        !active && "window-titlebar--inactive",
      )}
      data-tauri-drag-region
      onDoubleClick={isWindows ? () => void appWindow.toggleMaximize() : undefined}
    >
      <span className="window-titlebar-drag-region min-w-0 flex-1" data-tauri-drag-region />
      <span className="window-titlebar-title" data-tauri-drag-region>{title}</span>
      {isWindows ? (
        <nav className="ml-auto flex h-full" aria-label={t("common.windowControls")}>
          <WindowControl label={t("common.minimize")} onClick={() => void appWindow.minimize()}>
            <Minus size={15} strokeWidth={1.7} />
          </WindowControl>
          <WindowControl label={t("common.maximize")} onClick={() => void appWindow.toggleMaximize()}>
            <Maximize2 size={13} strokeWidth={1.7} />
          </WindowControl>
          <WindowControl
            label={t("common.close")}
            danger
            onClick={() => void (["settings", "accounts", "message-preview", "raw-message", "definition", "update"].includes(kind)
              ? appWindow.destroy()
              : appWindow.close())}
          >
            <X size={16} strokeWidth={1.7} />
          </WindowControl>
        </nav>
      ) : null}
    </header>
  );
}

function windowTitle(kind: WindowKind, t: (key: string) => string) {
  if (kind === "main") return "NextMail";
  if (kind === "composer") return `${t("composer.windowTitle")} — NextMail`;
  if (kind === "settings") return `${t("settings.title")} — NextMail`;
  if (kind === "accounts") return `${t("accounts.title")} — NextMail`;
  if (kind === "message-preview") return `${t("mail.previewWindowTitle")} — NextMail`;
  if (kind === "raw-message") return `${t("mail.sourceTitle")} — NextMail`;
  if (kind === "definition") return `${t("compositionLibrary.editorWindowTitle")} — NextMail`;
  return `${t("settings.group.updates")} — NextMail`;
}

function WindowControl({
  label,
  danger = false,
  onClick,
  children,
}: {
  label: string;
  danger?: boolean;
  onClick: () => void;
  children: React.ReactNode;
}) {
  return (
    <button
      type="button"
      className={cn(
        "grid h-full w-10 place-items-center border-0 bg-transparent text-muted-foreground outline-none transition-colors hover:bg-foreground/7 hover:text-foreground focus-visible:bg-foreground/7 focus-visible:text-foreground",
        danger && "hover:bg-[#e5484d] hover:text-white focus-visible:bg-[#e5484d] focus-visible:text-white",
      )}
      aria-label={label}
      title={label}
      onClick={onClick}
    >
      {children}
    </button>
  );
}
