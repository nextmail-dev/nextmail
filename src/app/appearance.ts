import { listen } from "@tauri-apps/api/event";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect } from "react";

import { api } from "./api";
import i18n from "./i18n";
import type { AppearancePreferences } from "./types";

export const appearanceQueryKey = ["preferences"] as const;

export const defaultPreferences: AppearancePreferences = {
  theme: "light",
  accentColor: "#2563eb",
  language: "zh-CN",
};

const LIGHT_PRIMARY_MAX_LUMINANCE = 0.175;
const DARK_PRIMARY_MIN_LUMINANCE = 0.24;

export function applyAppearance(
  preferences: AppearancePreferences,
  systemDark = globalThis.matchMedia?.("(prefers-color-scheme: dark)").matches ?? false,
) {
  const dark = preferences.theme === "dark" || (preferences.theme === "system" && systemDark);
  const primary = accessiblePrimary(preferences.accentColor, dark);
  document.documentElement.dataset.theme = preferences.theme;
  document.documentElement.lang = preferences.language;
  document.documentElement.style.setProperty(
    "--accent-color",
    preferences.accentColor,
  );
  document.documentElement.style.setProperty(
    "--primary",
    primary,
  );
  document.documentElement.style.setProperty(
    "--ring",
    primary,
  );
  document.documentElement.style.setProperty(
    "--primary-foreground",
    dark ? "#000000" : "#ffffff",
  );
}

export function accessiblePrimary(accentColor: string, dark: boolean) {
  const source = parseHexColor(accentColor);
  if (!source) return accentColor;
  const meetsTarget = (color: Rgb) => dark
    ? relativeLuminance(color) >= DARK_PRIMARY_MIN_LUMINANCE
    : relativeLuminance(color) <= LIGHT_PRIMARY_MAX_LUMINANCE;
  if (meetsTarget(source)) return accentColor.toLowerCase();

  const target: Rgb = dark ? [255, 255, 255] : [0, 0, 0];
  let low = 0;
  let high = 1;
  for (let index = 0; index < 12; index += 1) {
    const amount = (low + high) / 2;
    if (meetsTarget(mixRgb(source, target, amount))) high = amount;
    else low = amount;
  }
  return formatHexColor(mixRgb(source, target, high));
}

type Rgb = [number, number, number];

function parseHexColor(value: string): Rgb | null {
  const match = /^#([0-9a-f]{6})$/i.exec(value);
  if (!match) return null;
  return [0, 2, 4].map((offset) => Number.parseInt(match[1].slice(offset, offset + 2), 16)) as Rgb;
}

function mixRgb(source: Rgb, target: Rgb, amount: number): Rgb {
  return source.map((channel, index) => Math.round(
    channel + (target[index] - channel) * amount,
  )) as Rgb;
}

function formatHexColor(color: Rgb) {
  return `#${color.map((channel) => channel.toString(16).padStart(2, "0")).join("")}`;
}

function relativeLuminance(color: Rgb) {
  return color
    .map((channel) => channel / 255)
    .map((channel) => channel <= 0.04045
      ? channel / 12.92
      : ((channel + 0.055) / 1.055) ** 2.4)
    .reduce((sum, channel, index) => sum + channel * [0.2126, 0.7152, 0.0722][index], 0);
}

export function useAppearancePreferences() {
  const query = useQuery({
    queryKey: appearanceQueryKey,
    queryFn: api.getPreferences,
  });

  useEffect(() => {
    const preferences = query.data;
    if (!preferences) return;
    applyAppearance(preferences);
    void i18n.changeLanguage(preferences.language);
    if (preferences.theme !== "system" || !globalThis.matchMedia) return;
    const media = globalThis.matchMedia("(prefers-color-scheme: dark)");
    const reapplySystemAccent = () => applyAppearance(preferences, media.matches);
    media.addEventListener("change", reapplySystemAccent);
    return () => media.removeEventListener("change", reapplySystemAccent);
  }, [query.data]);

  return query;
}

export function useUpdateAppearancePreferences() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: api.setAppearancePreferences,
    onMutate: async (preferences) => {
      await queryClient.cancelQueries({ queryKey: appearanceQueryKey });
      const previous = queryClient.getQueryData<AppearancePreferences>(appearanceQueryKey);
      queryClient.setQueryData(appearanceQueryKey, preferences);
      return { previous };
    },
    onError: (_error, _preferences, context) => {
      if (context?.previous) {
        queryClient.setQueryData(appearanceQueryKey, context.previous);
      } else {
        queryClient.removeQueries({ queryKey: appearanceQueryKey, exact: true });
      }
    },
    onSuccess: (preferences) => {
      queryClient.setQueryData(appearanceQueryKey, preferences);
    },
  });
}

export function useAppearanceEventBridge() {
  const queryClient = useQueryClient();

  useEffect(() => {
    const unlisten = listen<AppearancePreferences>("appearance-preferences-changed", (event) => {
      queryClient.setQueryData(appearanceQueryKey, event.payload);
      applyAppearance(event.payload);
    });
    return () => { void unlisten.then((dispose) => dispose()); };
  }, [queryClient]);
}
