import { formatCommandError } from "@/app/commandErrors";
import { useEffect, useState } from "react";
import { useInfiniteQuery, useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Plus, Trash2, UsersRound } from "lucide-react";
import { useTranslation } from "react-i18next";

import { api, normalizeCommandError } from "@/app/api";
import type { ContactGroupDetail, ContactGroupDraft } from "@/app/types";
import { Alert } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Modal } from "@/components/ui/dialog";
import { TextField } from "@/components/ui/input";
import { OverlayScrollArea } from "@/components/ui/overlay-scroll-area";
import { Spinner } from "@/components/ui/spinner";
import { mailQueryKeys } from "@/features/mail/mail-query-keys";

export function ContactGroupManager({ accountId, onClose }: { accountId: string; onClose: () => void }) {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [dirty, setDirty] = useState(false);
  const [busy, setBusy] = useState(false);
  const [pendingAction, setPendingAction] = useState<(() => void) | null>(null);
  const groups = useQuery({
    queryKey: mailQueryKeys.contactGroups(accountId),
    queryFn: () => api.listContactGroups(accountId),
  });
  const detail = useQuery({
    queryKey: mailQueryKeys.contactGroup(accountId, selectedId ?? ""),
    queryFn: () => api.getContactGroup(accountId, selectedId!),
    enabled: Boolean(selectedId),
  });

  function navigate(action: () => void) {
    if (busy) return;
    if (dirty) setPendingAction(() => action);
    else action();
  }

  function finish() {
    setDirty(false);
    setSelectedId(null);
    void queryClient.invalidateQueries({ queryKey: mailQueryKeys.contactsForAccount(accountId) });
  }

  return (
    <>
      <Modal open onOpenChange={(open) => { if (!open) navigate(onClose); }} title={t("contactGroups.title")}
        closeLabel={t("common.close")} contentClassName="flex h-[min(640px,calc(100dvh-40px))] w-[min(820px,calc(100vw-40px))] flex-col">
        <p className="mt-2 text-sm text-muted-foreground">{t("contactGroups.description")}</p>
        <div className="mt-5 grid min-h-0 flex-1 grid-cols-[minmax(140px,1fr)_minmax(0,2fr)] gap-5">
          <div className="flex min-h-0 flex-col gap-3 border-r border-border pr-4">
            <Button variant="secondary" size="sm" disabled={busy || selectedId === ""} onClick={() => navigate(() => setSelectedId(""))}>
              <Plus size={16} />{t("contactGroups.create")}
            </Button>
            <OverlayScrollArea className="min-h-0 flex-1" contentClassName="space-y-1">
              {groups.isPending ? <Spinner /> : groups.isError ? (
                <Alert tone="danger">{t("errors.contact_group.read_failed")}<Button variant="ghost" onClick={() => void groups.refetch()}>{t("common.retry")}</Button></Alert>
              ) : !groups.data.length ? <p className="py-5 text-sm text-muted-foreground">{t("contactGroups.empty")}</p> : groups.data.map((group) => (
                <Button key={group.id} variant="list" disabled={busy} aria-pressed={selectedId === group.id}
                  className={`h-auto w-full justify-start gap-2 px-2 py-3 text-left ${selectedId === group.id ? "bg-selection text-foreground" : ""}`}
                  onClick={() => { if (selectedId !== group.id) navigate(() => setSelectedId(group.id)); }}>
                  <UsersRound size={17} className="shrink-0" />
                  <span className="min-w-0 flex-1">
                    <span className="block truncate" title={group.name}>{group.name}</span>
                    <span className="block text-xs font-normal text-muted-foreground">{t("contactGroups.memberCount", { count: group.memberCount })}</span>
                  </span>
                </Button>
              ))}
            </OverlayScrollArea>
          </div>
          {selectedId === null ? (
            <div className="grid place-items-center text-center text-sm text-muted-foreground"><p>{t("contactGroups.select")}</p></div>
          ) : selectedId && detail.isPending ? <div className="grid place-items-center"><Spinner /></div>
            : selectedId && detail.isError && !detail.data ? (
              <Alert tone="danger">{formatCommandError(t, normalizeCommandError(detail.error))}
                <Button variant="ghost" size="sm" onClick={() => void detail.refetch()}>{t("common.retry")}</Button>
              </Alert>
            ) : (
              <ContactGroupForm key={selectedId} accountId={accountId} initial={selectedId ? detail.data! : null}
                onDirtyChange={setDirty} onBusyChange={setBusy} onFinish={finish} />
            )}
        </div>
      </Modal>
      {pendingAction ? (
        <Modal open onOpenChange={(open) => { if (!open) setPendingAction(null); }} title={t("contactGroups.discardTitle")} closeLabel={t("common.close")}>
          <p className="my-5 text-sm text-muted-foreground">{t("contactGroups.discardDescription")}</p>
          <div className="flex justify-end gap-2">
            <Button variant="ghost" onClick={() => setPendingAction(null)}>{t("contactGroups.keepEditing")}</Button>
            <Button variant="danger" onClick={() => { setDirty(false); pendingAction(); setPendingAction(null); }}>{t("contactGroups.discard")}</Button>
          </div>
        </Modal>
      ) : null}
    </>
  );
}

function ContactGroupForm({ accountId, initial, onDirtyChange, onBusyChange, onFinish }: {
  accountId: string;
  initial: ContactGroupDetail | null;
  onDirtyChange: (dirty: boolean) => void;
  onBusyChange: (busy: boolean) => void;
  onFinish: () => void;
}) {
  const { t } = useTranslation();
  // Keep the revision belonging to this edit, even when background queries refresh.
  const [original] = useState(initial);
  const [name, setName] = useState(original?.group.name ?? "");
  const [memberIds, setMemberIds] = useState(() => new Set(original?.members.map((member) => member.id) ?? []));
  const [search, setSearch] = useState("");
  const [selectedOnly, setSelectedOnly] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const contacts = useInfiniteQuery({
    queryKey: mailQueryKeys.contactList(accountId, search.trim()),
    queryFn: ({ pageParam }) => api.listContacts(accountId, search.trim(), pageParam, 50),
    initialPageParam: null as string | null,
    getNextPageParam: (page) => page.nextCursor ?? undefined,
  });
  const save = useMutation({
    mutationFn: (draft: ContactGroupDraft) => api.saveContactGroup(accountId, original?.group.id ?? null, draft, original?.group.revision ?? null),
    onSuccess: onFinish,
  });
  const remove = useMutation({
    mutationFn: () => api.deleteContactGroup(accountId, original!.group.id, original!.group.revision),
    onSuccess: onFinish,
  });
  const busy = save.isPending || remove.isPending;
  const dirty = name !== (original?.group.name ?? "") || memberIds.size !== (original?.members.length ?? 0)
    || (original?.members.some((member) => !memberIds.has(member.id)) ?? false);
  useEffect(() => onDirtyChange(dirty), [dirty, onDirtyChange]);
  useEffect(() => { onBusyChange(busy); return () => onBusyChange(false); }, [busy, onBusyChange]);

  // Retain selected contacts across searches and pagination, including members outside the first page.
  const [knownMembers, setKnownMembers] = useState(() => new Map(original?.members.map((member) => [member.id, member]) ?? []));
  const visibleContacts = selectedOnly
    ? [...knownMembers.values()].filter((contact) => memberIds.has(contact.id)
      && `${contact.name}\n${contact.email}`.toLocaleLowerCase().includes(search.trim().toLocaleLowerCase()))
    : contacts.data?.pages.flatMap((page) => page.items) ?? [];
  const error = save.error ?? remove.error;

  return (
    <form className="flex min-h-0 min-w-0 flex-col gap-3" onSubmit={(event) => {
      event.preventDefault();
      if (!busy && !confirmDelete) save.mutate({ name, contactIds: [...memberIds] });
    }}>
      <TextField label={t("contactGroups.name")} value={name} maxLength={80} autoFocus disabled={busy}
        className="flex-none" onChange={(event) => setName(event.currentTarget.value)} />
      <TextField label={t("contacts.search")} type="search" value={search} className="flex-none"
        onKeyDown={(event) => { if (event.key === "Enter") event.preventDefault(); }}
        placeholder={t("contacts.searchPlaceholder")} onChange={(event) => setSearch(event.currentTarget.value)} />
      <div className="flex flex-wrap items-center justify-between gap-2">
        <span className="text-xs text-muted-foreground">{t("contactGroups.selectedCount", { count: memberIds.size })}</span>
        <Checkbox label={t("contactGroups.selectedOnly")} checked={selectedOnly} onCheckedChange={setSelectedOnly} />
      </div>
      <OverlayScrollArea className="min-h-0 flex-1" contentClassName="space-y-1">
        {!selectedOnly && contacts.isPending ? <Spinner /> : !selectedOnly && contacts.isError ? (
          <Alert tone="danger">{t("contacts.loadFailed")}<Button type="button" variant="ghost" size="sm" onClick={() => void contacts.refetch()}>{t("common.retry")}</Button></Alert>
        ) : !visibleContacts.length ? <p className="py-5 text-sm text-muted-foreground">{t("contacts.noResults")}</p> : visibleContacts.map((contact) => (
          <Checkbox key={contact.id} checked={memberIds.has(contact.id)} disabled={busy} label={contact.name}
            description={<span className="block truncate" title={contact.email}>{contact.email}</span>}
            className="[&>span>span:first-child]:break-all"
            onCheckedChange={(checked) => {
              setMemberIds((current) => { const next = new Set(current); if (checked) next.add(contact.id); else next.delete(contact.id); return next; });
              setKnownMembers((current) => new Map(current).set(contact.id, contact));
            }} />
        ))}
        {!selectedOnly && contacts.hasNextPage ? (
          <Button type="button" variant="ghost" className="w-full" loading={contacts.isFetchingNextPage} onClick={() => void contacts.fetchNextPage()}>{t("contacts.loadMore")}</Button>
        ) : null}
      </OverlayScrollArea>
      {error ? <Alert tone="danger" role="alert">{formatCommandError(t, normalizeCommandError(error))}</Alert> : null}
      {confirmDelete ? (
        <div className="space-y-2 rounded-md bg-destructive/10 p-3 text-sm">
          <p>{t("contactGroups.deleteDescription", { name: original?.group.name })}</p>
          <div className="flex justify-end gap-2">
            <Button type="button" size="sm" variant="ghost" disabled={busy} onClick={() => setConfirmDelete(false)}>{t("common.cancel")}</Button>
            <Button type="button" size="sm" variant="danger" loading={remove.isPending} onClick={() => remove.mutate()}>{t("common.delete")}</Button>
          </div>
        </div>
      ) : (
        <div className="flex flex-wrap items-center justify-end gap-2 border-t border-border pt-3">
          {original ? <Button type="button" size="icon" variant="ghost" className="mr-auto text-destructive" disabled={busy}
            aria-label={t("contactGroups.delete")} title={t("contactGroups.delete")} onClick={() => setConfirmDelete(true)}><Trash2 size={17} /></Button> : null}
          <Button type="button" variant="ghost" disabled={busy} onClick={onFinish}>{t("common.cancel")}</Button>
          <Button type="submit" loading={save.isPending} disabled={busy || !name.trim() || (!dirty && Boolean(original))}>{t("common.save")}</Button>
        </div>
      )}
    </form>
  );
}
