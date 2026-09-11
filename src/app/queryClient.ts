import { QueryClient } from "@tanstack/react-query";
import { normalizeCommandError } from "./commandErrors";

export function createAppQueryClient() {
  const client = new QueryClient({
    defaultOptions: {
      queries: {
        retry: (failures, error) => failures < 1 && normalizeCommandError(error).retryable,
        staleTime: 15_000,
        refetchOnWindowFocus: false,
      },
      // Finished mutations can contain whole bodies, attachments or passwords.
      // Retain only the mutation currently observed by the owning component.
      mutations: { retry: false, gcTime: 0 },
    },
  });
  client.setQueryDefaults(["message"], { gcTime: 0 });
  client.setQueryDefaults(["raw-message"], { gcTime: 0 });
  return client;
}
