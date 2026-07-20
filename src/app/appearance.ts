import { listen } from "@tauri-apps/api/event";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect } from "react";

import { api } from "./api";
import i18n from "./i18n";
import type { AppearancePreferences } from "./types";

export const appearanceQueryKey = ["preferences"] as const;

export const defaultPreferences: AppearancePreferences = {
  theme: "system",
  accentColor: "#2563eb",
  language: "zh-CN",
};

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

export function useAppearancePreferences() {
  const query = useQuery({
    queryKey: appearanceQueryKey,
    queryFn: api.getPreferences,
  });

  useEffect(() => {
    if (!query.data) return;
    applyAppearance(query.data);
    void i18n.changeLanguage(query.data.language);
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
