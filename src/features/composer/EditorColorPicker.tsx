import { useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { TextField } from "@/components/ui/input";
import { OverlayScrollArea } from "@/components/ui/overlay-scroll-area";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";

export function colorToHex(color: string | null): string | null {
  if (!color || color === "transparent" || color === "rgba(0, 0, 0, 0)") return null;
  if (/^#[\da-f]{6}$/i.test(color)) return color.toLowerCase();
  if (/^#[\da-f]{3}$/i.test(color)) return `#${color.slice(1).split("").map(c => c + c).join("")}`.toLowerCase();
  const rgb = /^rgb\(\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)\s*\)$/i.exec(color.trim());
  if (rgb && rgb.slice(1).every(n => Number(n) <= 255)) return `#${rgb.slice(1).map(n => Number(n).toString(16).padStart(2, "0")).join("")}`;
  return null;
}

export function hsvToHex(h: number, s: number, v: number): string {
  const f = (n: number) => {
    const k = (n + h / 60) % 6;
    return Math.round((v - v * s * Math.max(0, Math.min(k, 4 - k, 1))) * 255).toString(16).padStart(2, "0");
  };
  return `#${f(5)}${f(3)}${f(1)}`;
}

export function hexToHsv(hex: string) {
  const [r, g, b] = [1, 3, 5].map(i => Number.parseInt(hex.slice(i, i + 2), 16) / 255);
  const max = Math.max(r, g, b), min = Math.min(r, g, b), delta = max - min;
  const h = delta === 0 ? 0 : max === r ? ((g - b) / delta + 6) % 6 : max === g ? (b - r) / delta + 2 : (r - g) / delta + 4;
  return { h: h * 60, s: max === 0 ? 0 : delta / max, v: max };
}

export function EditorColorPicker({ label, icon, value, disabled, onSelect }: {
  label: string; icon: ReactNode; value: string | null; disabled?: boolean; onSelect: (color: string | null) => void;
}) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  return <Popover open={open && !disabled} onOpenChange={setOpen}>
    <PopoverTrigger asChild>
      <Button type="button" variant="ghost" size="icon" className="h-8 w-8 flex-col gap-0.5" aria-label={label} title={`${label}: ${value ?? t("composer.mixedStyle")}`} disabled={disabled}>
        {icon}<span className="h-1 w-4 rounded-sm border border-border" style={{ backgroundColor: value ?? "transparent" }} />
      </Button>
    </PopoverTrigger>
    <PopoverContent className="w-72 p-0" aria-label={label} onCloseAutoFocus={event => event.preventDefault()}>
      <OverlayScrollArea intrinsic className="max-h-[var(--radix-popover-content-available-height)]" contentClassName="p-3">
        <ColorPanel initialColor={colorToHex(value) ?? "#202124"} onSelect={(color) => { onSelect(color); setOpen(false); }} />
      </OverlayScrollArea>
    </PopoverContent>
  </Popover>;
}

function ColorPanel({ initialColor, onSelect }: { initialColor: string; onSelect: (color: string | null) => void }) {
  const { t } = useTranslation();
  const [hsv, setHsv] = useState(() => hexToHsv(initialColor));
  const [input, setInput] = useState(initialColor);
  const [invalid, setInvalid] = useState(false);
  const color = hsvToHex(hsv.h, hsv.s, hsv.v);
  const change = (next: typeof hsv) => { setHsv(next); setInput(hsvToHex(next.h, next.s, next.v)); setInvalid(false); };
  return <div className="flex flex-col gap-3">
    <div className="relative h-36 w-full touch-none rounded-none border border-border" aria-hidden="true"
      style={{ background: `linear-gradient(to top,#000,transparent),linear-gradient(to right,#fff,hsl(${hsv.h} 100% 50%))` }}
      onPointerDown={event => {
        if (event.button !== 0) return;
        event.preventDefault(); event.currentTarget.setPointerCapture(event.pointerId);
        const rect = event.currentTarget.getBoundingClientRect();
        change({ ...hsv, s: Math.max(0, Math.min(1, (event.clientX - rect.left) / rect.width)), v: 1 - Math.max(0, Math.min(1, (event.clientY - rect.top) / rect.height)) });
      }}
      onPointerMove={event => {
        if (!event.currentTarget.hasPointerCapture(event.pointerId)) return;
        const rect = event.currentTarget.getBoundingClientRect();
        change({ ...hsv, s: Math.max(0, Math.min(1, (event.clientX - rect.left) / rect.width)), v: 1 - Math.max(0, Math.min(1, (event.clientY - rect.top) / rect.height)) });
      }}
      onPointerUp={event => { if (event.currentTarget.hasPointerCapture(event.pointerId)) event.currentTarget.releasePointerCapture(event.pointerId); }}>
      <span className="pointer-events-none absolute size-3 -translate-x-1/2 -translate-y-1/2 rounded-full border-2 border-white shadow-[0_0_0_1px_#000]" style={{ left: `${hsv.s * 100}%`, top: `${(1 - hsv.v) * 100}%`, backgroundColor: color }} />
    </div>
    {([['h', 360, 'hue'], ['s', 100, 'saturation'], ['v', 100, 'brightness']] as const).map(([key, max, label]) => <label key={key} className="flex items-center gap-2 text-xs">
      <span className="w-12 shrink-0">{t(`composer.${label}`)}</span>
      <input type="range" min="0" max={max} value={key === "h" ? hsv.h : hsv[key] * 100} aria-label={t(`composer.${label}`)} className="composer-color-range min-w-0 flex-1" style={key === "h" ? { background: "linear-gradient(to right,red,#ff0,#0f0,#0ff,#00f,#f0f,red)" } : undefined}
        onChange={event => change({ ...hsv, [key]: Number(event.target.value) / (key === "h" ? 1 : 100) })} />
    </label>)}
    <div className="flex items-end gap-2"><span className="mb-1 size-8 shrink-0 rounded-none border border-border" style={{ backgroundColor: color }} /><TextField label={t("composer.colorHex")} value={input} error={invalid ? t("composer.invalidColor") : undefined}
      onChange={event => { setInput(event.target.value); const hex = colorToHex(event.target.value); if (hex) { setHsv(hexToHsv(hex)); setInvalid(false); } }}
      onKeyDown={event => { if (event.key === "Enter") { event.preventDefault(); const hex = colorToHex(input); if (hex) onSelect(hex); else setInvalid(true); } }} /></div>
    <div className="flex justify-between gap-2">
      <Button type="button" size="sm" variant="ghost" onClick={() => onSelect(null)}>{t("composer.colorDefault")}</Button>
      <Button type="button" size="sm" onClick={() => { const hex = colorToHex(input); if (hex) onSelect(hex); else setInvalid(true); }}>{t("composer.applyColor")}</Button>
    </div>
  </div>;
}
