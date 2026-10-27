import { useEffect, useId, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { ChevronDown, Check } from "lucide-react";
import { api } from "@/app/api";
import { formatCommandError } from "@/app/commandErrors";
import { normalizeCommandError } from "@/app/api";
import { Button } from "@/components/ui/button";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { OverlayScrollArea } from "@/components/ui/overlay-scroll-area";
import { TextField } from "@/components/ui/input";
import { fontLabel, quoteFontFamily, FONT_SIZES, parseFontSize } from "./editor-formatting";

let fontRequest: Promise<string[]> | undefined;
function loadFonts() {
  fontRequest ??= api.listSystemFonts().catch((error: unknown) => { fontRequest = undefined; throw error; });
  return fontRequest;
}

export function EditorFontPicker({ value, disabled, onSelect }: { value: string | null; disabled?: boolean; onSelect: (value: string | null) => void }) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  return (
    <Popover open={open && !disabled} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <Button type="button" variant="ghost" size="sm" className="h-8 w-40 justify-between border border-border bg-input px-2" disabled={disabled} aria-label={t("composer.fontFamily")}>
          <span className="truncate" style={{ fontFamily: value ?? undefined }}>{value ? fontLabel(value) : t("composer.mixedStyle")}</span><ChevronDown size={13} />
        </Button>
      </PopoverTrigger>
      <PopoverContent className="w-80" onCloseAutoFocus={(event) => event.preventDefault()}>
        <FontOptions value={value} onSelect={(font) => { onSelect(font); setOpen(false); }} />
      </PopoverContent>
    </Popover>
  );
}

function FontOptions({ value, onSelect }: { value: string | null; onSelect: (value: string | null) => void }) {
  const { t } = useTranslation();
  const [fonts, setFonts] = useState<string[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [attempt, setAttempt] = useState(0);
  const [query, setQuery] = useState("");
  const [active, setActive] = useState(0);
  const id = useId();
  const listRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    let disposed = false;
    setLoading(true);
    setError("");
    void loadFonts().then((names) => { if (!disposed) setFonts(names); }).catch((error: unknown) => {
      if (!disposed) setError(formatCommandError(t, normalizeCommandError(error)));
    }).finally(() => { if (!disposed) setLoading(false); });
    return () => { disposed = true; };
  }, [attempt, t]);
  const matches = fonts.filter(font => font.toLocaleLowerCase().includes(query.toLocaleLowerCase()));
  const visible = matches.slice(0, 100);
  useEffect(() => { listRef.current?.querySelector(`[data-font-index="${active}"]`)?.scrollIntoView?.({ block: "nearest" }); }, [active]);
  return <div className="flex flex-col gap-2">
    <TextField label={t("composer.searchFonts")} role="combobox" value={query} aria-expanded="true" aria-controls={id} aria-activedescendant={visible[active] ? `${id}-${active}` : undefined}
      onChange={(event) => { setQuery(event.target.value); setActive(0); }}
      onKeyDown={(event) => {
        if (event.key === "ArrowDown" || event.key === "ArrowUp") { event.preventDefault(); setActive(index => Math.max(0, Math.min(visible.length - 1, index + (event.key === "ArrowDown" ? 1 : -1)))); }
        if (event.key === "Enter" && visible[active]) { event.preventDefault(); onSelect(quoteFontFamily(visible[active])); }
      }} />
    <Button type="button" variant="ghost" size="sm" className="justify-start" onClick={() => onSelect(null)}>{t("composer.fontDefault")}</Button>
    {loading ? <p role="status" className="text-xs text-muted-foreground">{t("composer.loadingFonts")}</p> : null}
    {error ? <div role="alert" className="text-xs text-destructive">{error}<Button type="button" variant="ghost" size="sm" onClick={() => setAttempt(n => n + 1)}>{t("common.retry")}</Button></div> : null}
    <OverlayScrollArea className="h-64 max-h-[max(80px,calc(var(--radix-popover-content-available-height)-160px))]">
      <div role="listbox" id={id} aria-label={t("composer.fontFamily")} ref={listRef}>
        {visible.map((font, index) => <button type="button" role="option" aria-selected={Boolean(value && fontLabel(value).toLowerCase() === font.toLowerCase())} id={`${id}-${index}`} data-font-index={index} key={font}
          className={`flex w-full items-center justify-between gap-2 rounded-none px-2 py-2 text-left focus-visible:outline focus-visible:outline-1 focus-visible:-outline-offset-1 focus-visible:outline-ring ${index === active ? "bg-accent" : "hover:bg-accent"}`}
          onMouseEnter={() => setActive(index)} onClick={() => onSelect(quoteFontFamily(font))}>
          <span className="min-w-0"><span className="block truncate text-sm" style={{ fontFamily: quoteFontFamily(font) }}>{font}</span><span className="block truncate text-xs text-muted-foreground" style={{ fontFamily: quoteFontFamily(font) }}>{t("composer.fontPreview")}</span></span>
          {value && fontLabel(value).toLowerCase() === font.toLowerCase() ? <Check size={14} className="shrink-0" /> : null}
        </button>)}
      </div>
    </OverlayScrollArea>
    {!loading && !error && !matches.length ? <p className="text-xs text-muted-foreground">{t("composer.noFonts")}</p> : null}
    {matches.length > visible.length ? <p className="text-xs text-muted-foreground">{t("composer.moreFonts", { count: matches.length })}</p> : null}
  </div>;
}

export function EditorFontSize({ value, disabled, onSelect }: { value: string | null; disabled?: boolean; onSelect: (size: string | null) => void }) {
  const { t } = useTranslation();
  const display = value?.replace(/px$/, "") ?? "";
  const [input, setInput] = useState(display);
  const [error, setError] = useState(false);
  const [open, setOpen] = useState(false);
  const editing = useRef(false);
  const cancelled = useRef(false);
  const submitted = useRef(false);
  const id = useId();
  useEffect(() => { if (!editing.current) { setInput(display); setError(false); } }, [display]);
  const apply = () => {
    if (input === display) return true;
    const size = parseFontSize(input);
    if (!size) { setError(true); return false; }
    submitted.current = true;
    setError(false); onSelect(size); return true;
  };
  return <div className="relative">
    <div className="flex h-8 w-24 items-center rounded-md border border-border bg-input focus-within:ring-1 focus-within:ring-inset focus-within:ring-ring">
      <input aria-label={t("composer.fontSize")} title={t("composer.fontSizeHint")} aria-invalid={error} aria-describedby={error ? id : undefined} disabled={disabled} inputMode="decimal" value={input} placeholder={value === null ? t("composer.mixedStyle") : "14"}
        className="h-full w-16 min-w-0 bg-transparent px-2 text-sm outline-none disabled:opacity-50"
        onFocus={() => { editing.current = true; cancelled.current = false; submitted.current = false; }} onChange={event => { submitted.current = false; setInput(event.target.value); setError(false); }}
        onBlur={() => { editing.current = false; if (!cancelled.current && !submitted.current) apply(); }} onKeyDown={event => {
          if (event.key === "Enter") { event.preventDefault(); editing.current = false; apply(); }
          if (event.key === "Escape") { event.preventDefault(); cancelled.current = true; setInput(display); setError(false); event.currentTarget.blur(); }
        }} />
      <Popover open={open && !disabled} onOpenChange={setOpen}>
        <PopoverTrigger asChild><Button type="button" size="icon" variant="ghost" className="size-7 shrink-0" disabled={disabled} aria-label={t("composer.fontSizeOptions")}><ChevronDown size={13} /></Button></PopoverTrigger>
        <PopoverContent className="w-52 p-0" onCloseAutoFocus={event => event.preventDefault()}>
          <OverlayScrollArea intrinsic className="max-h-[var(--radix-popover-content-available-height)]" contentClassName="p-3">
          <Button type="button" size="sm" variant="ghost" className="mb-2 w-full" onClick={() => { onSelect(null); setInput("14"); setError(false); setOpen(false); }}>{t("composer.fontSizeDefault")}</Button>
          <div className="grid grid-cols-3 gap-1">{FONT_SIZES.map(size => <Button key={size} type="button" size="sm" variant={value === `${size}px` ? "secondary" : "ghost"} onClick={() => { setInput(String(size)); setError(false); onSelect(`${size}px`); setOpen(false); }}>{size}</Button>)}</div>
          </OverlayScrollArea>
        </PopoverContent>
      </Popover>
    </div>
    {error ? <span role="alert" id={id} className="absolute top-full left-0 z-10 mt-1 w-56 rounded-none border border-destructive/30 bg-popover p-2 text-xs text-destructive shadow-md">{t("composer.fontSizeHint")}</span> : null}
  </div>;
}
