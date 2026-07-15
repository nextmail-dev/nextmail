import { getCurrentWindow } from "@tauri-apps/api/window";
import { Maximize2, Minus, X } from "lucide-react";
import { useMemo } from "react";
import { useTranslation } from "react-i18next";

import { cn } from "@/lib/utils";
import { detectDesktopPlatform } from "@/app/platform";

export type WindowKind = "main" | "composer" | "settings";

interface WindowTitlebarProps {
  kind: WindowKind;
  title?: string;
}

export function WindowTitlebar({ kind, title }: WindowTitlebarProps) {
  const { t } = useTranslation();
  const isMac = useMemo(
    () => detectDesktopPlatform() === "macos",
    [],
  );
  const appWindow = getCurrentWindow();
  const background = kind === "main"
    ? "linear-gradient(90deg, var(--sidebar) 0 var(--shell-sidebar-width, 300px), var(--card) var(--shell-sidebar-width, 300px) 100%)"
    : kind === "settings"
      ? "linear-gradient(90deg, var(--sidebar) 0 240px, var(--card) 240px 100%)"
      : "var(--card)";

  return (
    <header
      className={cn(
        "window-titlebar fixed inset-x-0 top-0 z-[100] flex h-[var(--titlebar-height)] select-none items-center",
        isMac && "window-titlebar--mac",
      )}
      style={{ background }}
      data-tauri-drag-region
      onDoubleClick={() => {
        if (!isMac) void appWindow.toggleMaximize();
      }}
    >
      <span
        className="pointer-events-none min-w-0 flex-1 truncate px-4 text-xs font-medium text-muted-foreground"
        data-tauri-drag-region
      >
        {title}
      </span>
      {isMac ? null : (
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
            onClick={() => void (kind === "settings" ? appWindow.destroy() : appWindow.close())}
          >
            <X size={16} strokeWidth={1.7} />
          </WindowControl>
        </nav>
      )}
    </header>
  );
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
