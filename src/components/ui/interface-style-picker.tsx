import { Check, Inbox, Search, Send } from "lucide-react";
import { useId } from "react";
import { useTranslation } from "react-i18next";
import type { AppearancePreferences } from "@/app/types";
import { cn } from "@/lib/utils";

type InterfaceStyle = AppearancePreferences["style"];

export function InterfaceStylePicker({ value, onValueChange }: {
  value: InterfaceStyle;
  onValueChange: (style: InterfaceStyle) => void;
}) {
  const { t } = useTranslation();
  const name = useId();
  return <fieldset className="m-0 min-w-0 border-0 p-0">
    <legend className="mb-3 text-xs font-semibold">{t("preferences.interfaceStyle")}</legend>
    <div className="grid max-w-2xl grid-cols-2 gap-3">
      {(["nova", "lyra"] as const).map((style) => <label key={style} className="group min-w-0 cursor-default">
        <input type="radio" name={name} className="peer sr-only" value={style} checked={value === style}
          aria-label={style === "nova" ? "Nova" : "Lyra"} onChange={() => onValueChange(style)} />
        <span className={cn("block overflow-hidden rounded-lg border border-border bg-card p-2 group-hover:border-foreground/30 peer-focus-visible:ring-1 peer-focus-visible:ring-inset peer-focus-visible:ring-ring", value === style && "border-primary")}>
          <span data-interface-style={style} aria-hidden="true" className="block h-48 overflow-hidden rounded-md border border-border bg-background p-3">
            <span className="mb-2 flex items-center justify-between gap-2">
              <Inbox size={16} />
              <span data-icon-tone="current" className="flex h-[var(--control-height)] items-center gap-1 rounded-md bg-primary px-2 text-xs text-primary-foreground"><Send size={12} />{t("mail.compose")}</span>
            </span>
            <span className="mb-2 flex h-[var(--control-height)] items-center gap-2 rounded-md border border-border bg-input px-2"><Search size={12} /><span className="h-1 w-1/2 bg-muted-foreground/30" /></span>
            {[0, 1].map((row) => <span key={row} className={cn("flex h-[var(--control-height)] items-center gap-2 rounded-md px-2", row === 0 ? "bg-selection" : "bg-card")}>
              <span className="size-4 rounded-sm bg-primary/20" /><span className="h-1 w-3/5 bg-muted-foreground/40" />
            </span>)}
          </span>
          <span className="mt-3 flex items-center justify-between px-1 text-sm font-semibold">
            {style === "nova" ? "Nova" : "Lyra"}
            {value === style ? <Check size={16} aria-hidden="true" /> : null}
          </span>
          <span className="mt-1 block px-1 pb-1 text-xs leading-relaxed text-muted-foreground">{t(`preferences.${style}Description`)}</span>
        </span>
      </label>)}
    </div>
  </fieldset>;
}
