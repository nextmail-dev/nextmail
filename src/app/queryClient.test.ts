import { QueryObserver, MutationObserver } from "@tanstack/react-query";
import { describe, expect, it } from "vitest";
import { createAppQueryClient } from "./queryClient";

describe("application query lifetime", () => {
  it("releases bodies and finished mutation data after their observers leave", async () => {
    const client = createAppQueryClient();
    const query = new QueryObserver(client, { queryKey: ["message", "a", "b", "c"], queryFn: async () => "large body" });
    const stop = query.subscribe(() => undefined);
    await query.refetch();
    stop();
    const mutation = new MutationObserver(client, { mutationFn: async (password: string) => password });
    const stopMutation = mutation.subscribe(() => undefined);
    await mutation.mutate("secret");
    stopMutation();
    await new Promise((resolve) => setTimeout(resolve, 20));
    expect(client.getQueryCache().getAll()).toHaveLength(0);
    expect(client.getMutationCache().getAll()).toHaveLength(0);
    client.clear();
  });
  it("does not retry permanent failures or mutations", async () => {
    const client = createAppQueryClient();
    let attempts = 0;
    await expect(client.fetchQuery({ queryKey: ["accounts"], queryFn: async () => {
      attempts++;
      throw { code: "storage.accounts_corrupt", retryable: false, params: {} };
    } })).rejects.toMatchObject({ code: "storage.accounts_corrupt" });
    expect(attempts).toBe(1);
    expect(client.getDefaultOptions().mutations?.retry).toBe(false);
    client.clear();
  });
});
