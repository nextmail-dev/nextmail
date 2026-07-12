import { create } from "zustand";
import type { AppearancePreferences } from "./types";

export const defaultPreferences: AppearancePreferences = {
  theme: "system",
  accentColor: "#2563eb",
  language: "zh-CN",
};

interface AppearanceState {
  preferences: AppearancePreferences;
  setPreferences: (preferences: AppearancePreferences) => void;
}

export const useAppearanceStore = create<AppearanceState>((set) => ({
  preferences: defaultPreferences,
  setPreferences: (preferences) => set({ preferences }),
}));

export function applyAppearance(preferences: AppearancePreferences) {
  document.documentElement.dataset.theme = preferences.theme;
  document.documentElement.lang = preferences.language;
  document.documentElement.style.setProperty(
    "--primary",
    preferences.accentColor,
  );
  document.documentElement.style.setProperty(
    "--ring",
    preferences.accentColor,
  );
}
