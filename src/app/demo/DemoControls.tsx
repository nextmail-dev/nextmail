import { formatCommandError } from "@/app/commandErrors";
import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { api, normalizeCommandError } from "../api";
import { useAppearancePreferences, useUpdateAppearancePreferences } from "../appearance";
import type { LanguagePreference } from "../types";
import { Button } from "@/components/ui/button";
import { Modal } from "@/components/ui/dialog";
import { Alert } from "@/components/ui/alert";
import { Stack } from "@/components/ui/layout";
import { Heading, Text } from "@/components/ui/typography";
import { isDemoMode } from "./session";

export function DemoEntryTitle() {
  const { t } = useTranslation();
  const title = useRef<HTMLButtonElement>(null);
  const clicks = useRef({ count: 0, time: 0 });
  const [open, setOpen] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<import("@/app/types").CommandError | string | null>(null);
  useEffect(() => {
    const resetOutside = (event: MouseEvent) => {
      if (!title.current?.contains(event.target as Node)) clicks.current.count = 0;
    };
    document.addEventListener("click", resetOutside, true);
    return () => document.removeEventListener("click", resetOutside, true);
  }, []);
  const click = () => {
    const now = Date.now();
    clicks.current.count = now - clicks.current.time > 2000 ? 1 : clicks.current.count + 1;
    clicks.current.time = now;
    if (clicks.current.count > 10) {
      clicks.current.count = 0;
      setError(null);
      setOpen(true);
    }
  };
  async function enter() {
    setBusy(true);
    setError(null);
    try { await api.enterDemoMode(); }
    catch (cause) { setError(normalizeCommandError(cause)); }
    finally { setBusy(false); }
  }
  return <>
    <Heading level={2}><button ref={title} type="button" className="cursor-default rounded-sm text-left focus-visible:outline focus-visible:outline-1 focus-visible:-outline-offset-1 focus-visible:outline-ring" onClick={click}>NextMail</button></Heading>
    <Modal open={open} onOpenChange={(value) => { if (!busy) setOpen(value); }} title={t("demo.enterTitle")} closeLabel={t("common.close")}>
      <Stack className="pt-4" gap="md">
        <Text>{t("demo.enterDescription")}</Text>
        {error && <Alert tone="danger">{formatCommandError(t, error)}</Alert>}
        <div className="flex justify-end gap-2">
          <Button variant="ghost" disabled={busy} onClick={() => setOpen(false)}>{t("common.cancel")}</Button>
          <Button disabled={busy} onClick={() => void enter()}>{t("demo.enter")}</Button>
        </div>
      </Stack>
    </Modal>
  </>;
}

export function DemoLanguageDialog() {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const preferences = useAppearancePreferences();
  const mutation = useUpdateAppearancePreferences();
  const [open, setOpen] = useState(false);
  useEffect(() => {
    if (!isDemoMode()) return;
    const shortcut = (event: KeyboardEvent) => {
      if ((event.ctrlKey || event.metaKey) && !event.altKey && !event.shiftKey && event.key.toLowerCase() === "l") {
        event.preventDefault();
        event.stopPropagation();
        if (!event.repeat) setOpen(true);
      }
    };
    window.addEventListener("keydown", shortcut, true);
    const unlisten = "__TAURI_INTERNALS__" in globalThis
      ? listen("demo-language-requested", () => setOpen(true))
      : null;
    return () => {
      window.removeEventListener("keydown", shortcut, true);
      void unlisten?.then((dispose) => dispose());
    };
  }, []);
  async function choose(language: LanguagePreference) {
    if (!preferences.data) return;
    try {
      await mutation.mutateAsync({ ...preferences.data, language });
      await queryClient.invalidateQueries();
      setOpen(false);
    } catch { /* The mutation exposes the error below. */ }
  }
  return <Modal open={open} onOpenChange={setOpen} title={t("demo.languageTitle")} closeLabel={t("common.close")}>
    <Stack className="pt-4" gap="md">
      <Text>{t("demo.languageDescription")}</Text>
      <Button variant={preferences.data?.language === "zh-CN" ? "primary" : "secondary"} disabled={mutation.isPending} onClick={() => void choose("zh-CN")}>简体中文</Button>
      <Button variant={preferences.data?.language === "en-US" ? "primary" : "secondary"} disabled={mutation.isPending} onClick={() => void choose("en-US")}>English</Button>
      {mutation.isError && <Alert tone="danger">{t("common.unexpectedError")}</Alert>}
    </Stack>
  </Modal>;
}
