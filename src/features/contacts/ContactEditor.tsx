import { formatCommandError } from "@/app/commandErrors";
import { useEffect, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";

import { api, normalizeCommandError } from "@/app/api";
import type { ContactDraft, ContactSummary } from "@/app/types";
import { Alert } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Modal } from "@/components/ui/dialog";
import { TextField } from "@/components/ui/input";
import { Inline } from "@/components/ui/layout";
import { Spinner } from "@/components/ui/spinner";
import { mailQueryKeys, messageQueryKeys } from "@/features/mail/mail-query-keys";

export type ContactEditorState =
  | { mode: "create" }
  | { mode: "edit"; contact: ContactSummary }
  | null;

export function ContactEditor({
  state,
  busy,
  errorCode,
  onClose,
  onCreate,
  onUpdate,
}: {
  state: ContactEditorState;
  busy: boolean;
  errorCode: import("@/app/types").CommandError | string | null;
  onClose: () => void;
  onCreate?: (draft: ContactDraft) => void;
  onUpdate: (contact: ContactSummary, name: string) => void;
}) {
  const { t } = useTranslation();
  const [name, setName] = useState("");
  const [email, setEmail] = useState("");

  useEffect(() => {
    setName(state?.mode === "edit" ? state.contact.name : "");
    setEmail(state?.mode === "edit" ? state.contact.email : "");
  }, [state]);

  if (!state) return null;
  return (
    <Modal
      open
      onOpenChange={(open) => { if (!open && !busy) onClose(); }}
      title={state.mode === "create" ? t("contacts.add") : t("contacts.edit")}
      closeLabel={t("common.close")}
    >
      <form
        className="mt-5 space-y-4"
        onSubmit={(event) => {
          event.preventDefault();
          if (state.mode === "create") onCreate?.({ name, email });
          else onUpdate(state.contact, name);
        }}
      >
        <TextField
          label={t("contacts.name")}
          value={name}
          maxLength={160}
          autoFocus
          onChange={(event) => setName(event.currentTarget.value)}
        />
        <TextField
          label={t("contacts.email")}
          type="email"
          value={email}
          disabled={state.mode === "edit"}
          hint={state.mode === "edit" ? t("contacts.emailImmutable") : undefined}
          onChange={(event) => setEmail(event.currentTarget.value)}
        />
        {errorCode ? (
          <Alert tone="danger">{formatCommandError(t, errorCode)}</Alert>
        ) : null}
        <Inline className="flex-wrap justify-end pt-2">
          <Button type="button" variant="ghost" disabled={busy} onClick={onClose}>{t("common.cancel")}</Button>
          <Button type="submit" loading={busy} disabled={!name.trim() || (state.mode === "create" && !email.trim())}>
            {t("common.save")}
          </Button>
        </Inline>
      </form>
    </Modal>
  );
}

export function DirectContactEditor({
  accountId,
  contactId,
  onClose,
}: {
  accountId: string;
  contactId: string;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const [errorCode, setErrorCode] = useState<import("@/app/types").CommandError | string | null>(null);
  const contactQuery = useQuery({
    queryKey: mailQueryKeys.contactSummary(accountId, contactId),
    queryFn: () => api.getContactSummary(accountId, contactId),
    enabled: Boolean(accountId && contactId),
  });
  const updateMutation = useMutation({
    mutationFn: ({ contact, name }: { contact: ContactSummary; name: string }) =>
      api.updateContactName(accountId, contact.id, name, contact.revision),
    onSuccess: async (contact) => {
      queryClient.setQueryData(mailQueryKeys.contactSummary(accountId, contact.id), contact);
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: mailQueryKeys.contactsForAccount(accountId) }),
        queryClient.invalidateQueries({ queryKey: mailQueryKeys.messagesForAccount(accountId) }),
        queryClient.invalidateQueries({ queryKey: messageQueryKeys.account(accountId) }),
      ]);
      onClose();
    },
    onError: (error) => setErrorCode(normalizeCommandError(error)),
  });

  useEffect(() => setErrorCode(null), [accountId, contactId]);

  if (!contactId) return null;
  if (contactQuery.isPending) {
    return (
      <Modal open onOpenChange={(open) => { if (!open) onClose(); }} title={t("contacts.edit")} closeLabel={t("common.close")}>
        <div className="grid min-h-28 place-items-center"><Spinner size={22} /></div>
      </Modal>
    );
  }
  if (contactQuery.isError || !contactQuery.data) {
    const code = contactQuery.error ?? "common.unexpected_error";
    return (
      <Modal open onOpenChange={(open) => { if (!open) onClose(); }} title={t("contacts.edit")} closeLabel={t("common.close")}>
        <div className="mt-5 space-y-4">
          <Alert tone="danger">{formatCommandError(t, code)}</Alert>
          <Inline className="justify-end">
            <Button variant="secondary" onClick={() => void contactQuery.refetch()}>{t("common.retry")}</Button>
          </Inline>
        </div>
      </Modal>
    );
  }
  return (
    <ContactEditor
      state={{ mode: "edit", contact: contactQuery.data }}
      busy={updateMutation.isPending}
      errorCode={errorCode}
      onClose={onClose}
      onUpdate={(contact, name) => updateMutation.mutate({ contact, name })}
    />
  );
}
