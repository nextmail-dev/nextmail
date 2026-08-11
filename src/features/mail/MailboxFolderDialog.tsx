import { useEffect, useMemo, useState, type FormEvent } from "react";
import { useTranslation } from "react-i18next";

import type { MailboxSummary } from "@/app/types";
import { Button } from "@/components/ui/button";
import { Modal } from "@/components/ui/dialog";
import { Form, Inline, Stack } from "@/components/ui/layout";
import { TextField } from "@/components/ui/input";
import { SelectField } from "@/components/ui/select";
import { Text } from "@/components/ui/typography";
import type { MailboxHierarchyItem } from "./MailboxPane";

export type MailboxDialogAction =
  | { kind: "create"; parent: MailboxSummary | null }
  | { kind: "rename"; mailbox: MailboxSummary; displayName: string }
  | { kind: "move"; mailbox: MailboxSummary; displayName: string }
  | { kind: "delete"; mailbox: MailboxSummary; displayName: string };

interface MailboxFolderDialogProps {
  action: MailboxDialogAction | null;
  hierarchy: MailboxHierarchyItem[];
  busy: boolean;
  onClose: () => void;
  onCreate: (parentMailboxId: string | null, name: string) => Promise<void>;
  onRename: (mailboxId: string, name: string) => Promise<void>;
  onMove: (mailboxId: string, destinationParentMailboxId: string | null) => Promise<void>;
  onDelete: (mailboxId: string) => Promise<void>;
}

export function MailboxFolderDialog({
  action,
  hierarchy,
  busy,
  onClose,
  onCreate,
  onRename,
  onMove,
  onDelete,
}: MailboxFolderDialogProps) {
  const { t } = useTranslation();
  const [name, setName] = useState("");
  const [destinationId, setDestinationId] = useState("__root__");

  useEffect(() => {
    setName(action?.kind === "rename" ? action.displayName : "");
    setDestinationId("__root__");
  }, [action]);

  const moveOptions = useMemo(() => {
    if (action?.kind !== "move") return [];
    const sourceItem = hierarchy.find(({ mailbox }) => mailbox.id === action.mailbox.id);
    const sourcePath = sourceItem ? [...sourceItem.ancestorIds, sourceItem.mailbox.id] : [];
    const currentParentId = sourceItem?.ancestorIds[
      sourceItem.ancestorIds.length - 1
    ] ?? null;
    return [
      ...(currentParentId === null
        ? []
        : [{ value: "__root__", label: t("mail.folderRoot") }]),
      ...hierarchy
        .filter(({ mailbox }) =>
          Boolean(mailbox.delimiter)
          && !sourcePath.includes(mailbox.id)
          && mailbox.id !== currentParentId)
        .map(({ mailbox, depth, displayName }) => ({
          value: mailbox.id,
          label: `${"　".repeat(depth)}${mailbox.role === "other"
            ? displayName
            : t(`mailboxNames.${mailbox.role}`)}`,
        })),
    ];
  }, [action, hierarchy, t]);

  useEffect(() => {
    if (action?.kind !== "move") return;
    if (!moveOptions.some(({ value }) => value === destinationId)) {
      setDestinationId(moveOptions[0]?.value ?? "");
    }
  }, [action, destinationId, moveOptions]);

  if (!action) return null;
  const title = t(`mail.folderDialog.${action.kind}.title`);

  async function submit(event: FormEvent) {
    event.preventDefault();
    if (action?.kind === "create") {
      await onCreate(action.parent?.id ?? null, name);
    } else if (action?.kind === "rename") {
      await onRename(action.mailbox.id, name);
    } else if (action?.kind === "move") {
      await onMove(action.mailbox.id, destinationId === "__root__" ? null : destinationId);
    } else if (action?.kind === "delete") {
      await onDelete(action.mailbox.id);
    }
    onClose();
  }

  return (
    <Modal
      open
      onOpenChange={(open) => {
        if (!open && !busy) onClose();
      }}
      title={title}
      closeLabel={t("common.close")}
    >
      <Form
        className="pt-5"
        onSubmit={(event) => void submit(event).catch(() => undefined)}
      >
        <Stack gap="md">
          {action.kind === "create" || action.kind === "rename" ? (
            <TextField
              autoFocus
              label={t("mail.folderName")}
              value={name}
              onChange={(event) => setName(event.target.value)}
              placeholder={t("mail.folderNamePlaceholder")}
              disabled={busy}
            />
          ) : null}
          {action.kind === "move" ? (
            <SelectField
              label={t("mail.folderDestination")}
              value={destinationId}
              options={moveOptions}
              onValueChange={setDestinationId}
              disabled={busy}
            />
          ) : null}
          {action.kind === "delete" ? (
            <Text>{t("mail.folderDialog.delete.description", { folder: action.displayName })}</Text>
          ) : null}
          {action.kind === "create" && action.parent ? (
            <Text>{t("mail.folderDialog.create.parent", {
              folder: action.parent.role === "other"
                ? action.parent.name
                : t(`mailboxNames.${action.parent.role}`),
            })}</Text>
          ) : null}
          <Inline className="flex-wrap justify-end">
            <Button type="button" variant="ghost" disabled={busy} onClick={onClose}>
              {t("common.cancel")}
            </Button>
            <Button
              type="submit"
              variant={action.kind === "delete" ? "danger" : "primary"}
              loading={busy}
              disabled={
                ((action.kind === "create" || action.kind === "rename") && !name.trim())
                || (action.kind === "move" && moveOptions.length === 0)
              }
            >
              {t(action.kind === "delete" ? "common.delete" : "common.confirm")}
            </Button>
          </Inline>
        </Stack>
      </Form>
    </Modal>
  );
}
