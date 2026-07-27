import { useMutation, useQueryClient } from "@tanstack/react-query";

import { api, normalizeCommandError } from "@/app/api";
import { mailQueryKeys } from "../mail-query-keys";

type MailboxAction =
  | { kind: "create"; parentMailboxId: string | null; name: string }
  | { kind: "rename"; mailboxId: string; name: string }
  | { kind: "move"; mailboxId: string; destinationParentMailboxId: string | null }
  | { kind: "delete"; mailboxId: string }
  | { kind: "markAllRead"; mailboxId: string }
  | { kind: "reorder"; orderedMailboxIds: string[] };

interface UseMailboxActionsOptions {
  accountId: string;
  onError: (code: string) => void;
}

export function useMailboxActions({ accountId, onError }: UseMailboxActionsOptions) {
  const queryClient = useQueryClient();
  const mutation = useMutation({
    mutationFn: async (action: MailboxAction) => {
      if (action.kind === "create") {
        return api.createMailbox(accountId, action.parentMailboxId, action.name);
      }
      if (action.kind === "rename") {
        return api.renameMailbox(accountId, action.mailboxId, action.name);
      }
      if (action.kind === "move") {
        return api.moveMailbox(accountId, action.mailboxId, action.destinationParentMailboxId);
      }
      if (action.kind === "delete") {
        return api.deleteMailbox(accountId, action.mailboxId);
      }
      if (action.kind === "markAllRead") {
        return api.markMailboxAllRead(accountId, action.mailboxId);
      }
      return api.reorderMailboxes(accountId, action.orderedMailboxIds);
    },
    onSuccess: async () => {
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: mailQueryKeys.mailboxes(accountId) }),
        queryClient.invalidateQueries({ queryKey: mailQueryKeys.messagesForAccount(accountId) }),
      ]);
    },
    onError: (error) => onError(normalizeCommandError(error).code),
  });

  return {
    busy: mutation.isPending,
    createMailbox: (parentMailboxId: string | null, name: string) =>
      mutation.mutateAsync({ kind: "create", parentMailboxId, name }),
    renameMailbox: (mailboxId: string, name: string) =>
      mutation.mutateAsync({ kind: "rename", mailboxId, name }),
    moveMailbox: (mailboxId: string, destinationParentMailboxId: string | null) =>
      mutation.mutateAsync({ kind: "move", mailboxId, destinationParentMailboxId }),
    deleteMailbox: (mailboxId: string) =>
      mutation.mutateAsync({ kind: "delete", mailboxId }),
    markMailboxAllRead: (mailboxId: string) =>
      mutation.mutateAsync({ kind: "markAllRead", mailboxId }),
    reorderMailboxes: (orderedMailboxIds: string[]) =>
      mutation.mutateAsync({ kind: "reorder", orderedMailboxIds }),
  };
}
