import { useEffect, useMemo, useRef, useState, type ReactElement, type UIEvent } from "react";
import { useInfiniteQuery, useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Clock3, Copy, Mail, Pencil, Plus, Search, Send, Trash2, UserRound, UsersRound } from "lucide-react";
import { useTranslation } from "react-i18next";

import { api, normalizeCommandError } from "@/app/api";
import type { ContactDraft, ContactSummary, NotificationNavigationTarget } from "@/app/types";
import { Alert } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuSeparator,
  ContextMenuTrigger,
} from "@/components/ui/context-menu";
import { EmptyState } from "@/components/ui/empty-state";
import { TextField } from "@/components/ui/input";
import { Inline, Page, Stack } from "@/components/ui/layout";
import { Modal } from "@/components/ui/dialog";
import { OverlayScrollArea } from "@/components/ui/overlay-scroll-area";
import { ResizeHandle } from "@/components/ui/resize-handle";
import { SearchField } from "@/components/ui/search-field";
import { Spinner } from "@/components/ui/spinner";
import { Heading, Text } from "@/components/ui/typography";
import { useListSelection } from "@/components/ui/use-list-selection";
import { cn } from "@/lib/utils";
import { mailQueryKeys, messageQueryKeys } from "@/features/mail/mail-query-keys";
import { ContactIdentity, ContactInitial, writeClipboardText } from "./ContactIdentity";

interface ContactsWorkspaceProps {
  accountId: string;
  listPaneWidth: number;
  listPaneMax: number;
  onListPaneWidthChange: (width: number) => void;
  onNavigateToMessage: (target: NotificationNavigationTarget) => void;
  requestedContactId?: string;
  requestedContactEdit?: { contactId: string; requestId: number } | null;
}

type EditorState = { mode: "create" } | { mode: "edit"; contact: ContactSummary } | null;

export function ContactsWorkspace({
  accountId,
  listPaneWidth,
  listPaneMax,
  onListPaneWidthChange,
  onNavigateToMessage,
  requestedContactId,
  requestedContactEdit,
}: ContactsWorkspaceProps) {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const [selectedContactId, setSelectedContactId] = useState("");
  const [searchInput, setSearchInput] = useState("");
  const [submittedSearch, setSubmittedSearch] = useState("");
  const [editor, setEditor] = useState<EditorState>(null);
  const [operationError, setOperationError] = useState<string | null>(null);
  const handledEditRequestRef = useRef(0);

  useEffect(() => {
    setSelectedContactId("");
    setSearchInput("");
    setSubmittedSearch("");
    setEditor(null);
    setOperationError(null);
  }, [accountId]);

  useEffect(() => {
    if (requestedContactId) setSelectedContactId(requestedContactId);
  }, [requestedContactId]);

  const contactsQuery = useInfiniteQuery({
    queryKey: mailQueryKeys.contactList(accountId, submittedSearch),
    queryFn: ({ pageParam }) => api.listContacts(accountId, submittedSearch, pageParam, 50),
    initialPageParam: null as string | null,
    getNextPageParam: (lastPage) => lastPage.nextCursor,
    enabled: Boolean(accountId),
  });
  const contacts = useMemo(
    () => contactsQuery.data?.pages.flatMap((page) => page.items) ?? [],
    [contactsQuery.data],
  );
  const contactIds = contacts.map((contact) => contact.id);
  const selection = useListSelection({
    itemIds: contactIds,
    primaryId: selectedContactId,
    resetKey: `${accountId}:${submittedSearch}`,
    onPrimaryChange: setSelectedContactId,
  });
  const selectedContactIdSet = new Set(selection.orderedSelectedIds);
  const selectedContacts = contacts.filter((contact) => selectedContactIdSet.has(contact.id));
  const total = contactsQuery.data?.pages[0]?.total ?? 0;
  const readingPreferences = useQuery({
    queryKey: ["reading-preferences"],
    queryFn: api.getReadingPreferences,
  });
  const autoLoadMoreContacts = readingPreferences.data?.autoLoadMoreContacts ?? true;

  function loadNextPageNearEnd(event: UIEvent<HTMLDivElement>) {
    if (!autoLoadMoreContacts || !contactsQuery.hasNextPage || contactsQuery.isFetchingNextPage) return;
    const viewport = event.currentTarget;
    if (viewport.scrollHeight - viewport.scrollTop - viewport.clientHeight <= 160) {
      void contactsQuery.fetchNextPage();
    }
  }

  const detailQuery = useQuery({
    queryKey: mailQueryKeys.contactDetail(accountId, selectedContactId),
    queryFn: () => api.getContactDetail(accountId, selectedContactId),
    enabled: Boolean(accountId && selectedContactId),
  });

  useEffect(() => {
    if (!requestedContactEdit || handledEditRequestRef.current === requestedContactEdit.requestId) return;
    const contact = detailQuery.data?.contact;
    if (!contact || contact.id !== requestedContactEdit.contactId) return;
    handledEditRequestRef.current = requestedContactEdit.requestId;
    setOperationError(null);
    setEditor({ mode: "edit", contact });
  }, [detailQuery.data, requestedContactEdit]);

  function editContact(contact: ContactSummary) {
    setSelectedContactId(contact.id);
    setOperationError(null);
    setEditor({ mode: "edit", contact });
  }

  const createMutation = useMutation({
    mutationFn: (draft: ContactDraft) => api.createContact(accountId, draft),
    onSuccess: async (contact) => {
      setEditor(null);
      setSelectedContactId(contact.id);
      setOperationError(null);
      await queryClient.invalidateQueries({ queryKey: mailQueryKeys.contactsForAccount(accountId) });
    },
    onError: (error) => setOperationError(normalizeCommandError(error).code),
  });
  const updateMutation = useMutation({
    mutationFn: ({ contact, name }: { contact: ContactSummary; name: string }) =>
      api.updateContactName(accountId, contact.id, name, contact.revision),
    onSuccess: async (contact) => {
      setEditor(null);
      setOperationError(null);
      queryClient.setQueryData(
        mailQueryKeys.contactDetail(accountId, contact.id),
        (current: Awaited<ReturnType<typeof api.getContactDetail>> | undefined) => current
          ? { ...current, contact }
          : current,
      );
      await queryClient.invalidateQueries({ queryKey: mailQueryKeys.contactsForAccount(accountId) });
    },
    onError: (error) => setOperationError(normalizeCommandError(error).code),
  });
  const composeMutation = useMutation({
    mutationFn: (contactId: string) => api.openContactComposer(accountId, contactId),
    onError: (error) => setOperationError(normalizeCommandError(error).code),
  });
  const deleteMutation = useMutation({
    mutationFn: (contactIds: string[]) => api.deleteContacts(accountId, contactIds),
    onSuccess: async (_result, deletedContactIds) => {
      const deleted = new Set(deletedContactIds);
      if (deleted.has(selectedContactId)) {
        const selectedIndex = contacts.findIndex((contact) => contact.id === selectedContactId);
        const next = contacts.slice(selectedIndex + 1).find((contact) => !deleted.has(contact.id))
          ?? contacts.slice(0, Math.max(0, selectedIndex)).reverse().find((contact) => !deleted.has(contact.id));
        setSelectedContactId(next?.id ?? "");
      }
      selection.clear();
      setOperationError(null);
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: mailQueryKeys.contactsForAccount(accountId) }),
        queryClient.invalidateQueries({ queryKey: mailQueryKeys.messagesForAccount(accountId) }),
        queryClient.invalidateQueries({ queryKey: messageQueryKeys.account(accountId) }),
      ]);
    },
    onError: (error) => setOperationError(normalizeCommandError(error).code),
  });

  return (
    <Page
      className="grid min-h-0 bg-card"
      style={{ gridTemplateColumns: `${listPaneWidth}px 0 minmax(360px,1fr)` }}
    >
      <Page className="flex min-h-0 flex-col border-r border-border/70 bg-card">
        <Stack className="border-b border-border px-5 pt-5 pb-4" gap="md">
          <Inline className="justify-between">
            <Stack gap="xs">
              <Heading level={2}>{t("contacts.title")}</Heading>
              <Text className="text-xs">
                {selection.orderedSelectedIds.length > 1
                  ? t("contacts.selectedCount", { count: selection.orderedSelectedIds.length })
                  : t("contacts.count", { count: total })}
              </Text>
            </Stack>
            <Button
              size="icon"
              variant="ghost"
              aria-label={t("contacts.add")}
              title={t("contacts.add")}
              onClick={() => {
                setOperationError(null);
                setEditor({ mode: "create" });
              }}
            >
              <Plus size={18} />
            </Button>
          </Inline>
          <SearchField
            className="w-full"
            value={searchInput}
            placeholder={t("contacts.searchPlaceholder")}
            aria-label={t("contacts.search")}
            clearLabel={t("contacts.clearSearch")}
            submitLabel={t("contacts.search")}
            onValueChange={(value) => {
              setSearchInput(value);
              if (!value) setSubmittedSearch("");
            }}
            onSubmit={() => {
              setSelectedContactId("");
              setSubmittedSearch(searchInput.trim());
            }}
          />
        </Stack>

        {contactsQuery.isPending ? (
          <EmptyState className="m-auto" icon={<UsersRound size={21} />} title={t("contacts.loading")} />
        ) : contactsQuery.isError ? (
          <EmptyState
            className="m-auto"
            icon={<UsersRound size={21} />}
            title={t("contacts.loadFailed")}
            action={<Button variant="secondary" onClick={() => void contactsQuery.refetch()}>{t("common.retry")}</Button>}
          />
        ) : contacts.length === 0 ? (
          <EmptyState
            className="m-auto"
            icon={submittedSearch ? <Search size={21} /> : <UsersRound size={21} />}
            title={submittedSearch ? t("contacts.noResults") : t("contacts.empty")}
            description={submittedSearch ? undefined : t("contacts.emptyDescription")}
            action={submittedSearch ? undefined : (
              <Button onClick={() => setEditor({ mode: "create" })}><Plus size={16} />{t("contacts.add")}</Button>
            )}
          />
        ) : (
          <OverlayScrollArea
            className="min-h-0 flex-1"
            contentClassName="gap-0"
            viewportClassName="pr-2"
            onViewportScroll={loadNextPageNearEnd}
          >
            {contacts.map((contact) => {
              const operationContacts = selectedContactIdSet.has(contact.id) ? selectedContacts : [contact];
              return (
                <ContactActionsContextMenu
                  key={contact.id}
                  contact={contact}
                  selectionCount={operationContacts.length}
                  pending={deleteMutation.isPending || composeMutation.isPending}
                  onCompose={() => composeMutation.mutate(contact.id)}
                  onEdit={() => editContact(contact)}
                  onDelete={() => deleteMutation.mutate(operationContacts.map((item) => item.id))}
                  onCopyError={() => setOperationError("common.unexpected_error")}
                >
                  <div
                    onContextMenu={() => selection.selectForContextMenu(contact.id)}
                  >
                    <button
                      type="button"
                      aria-pressed={selection.isSelected(contact.id)}
                      className={cn(
                        "relative flex w-full cursor-pointer items-center gap-3 bg-card px-6 py-4 text-left outline-none transition-colors hover:bg-muted/75 focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-ring/60",
                        selection.isSelected(contact.id) && "bg-selection before:absolute before:inset-y-0 before:left-0 before:w-[3px] before:rounded-r-full before:bg-primary hover:bg-selection",
                      )}
                      onClick={(event) => selection.select(contact.id, event)}
                    >
                      <ContactInitial name={contact.name} className="size-10" />
                      <span className="block min-w-0 flex-1">
                        <span className="block truncate text-sm font-semibold text-foreground">{contact.name}</span>
                        <span className="block truncate pt-1 text-xs text-muted-foreground">{contact.email}</span>
                      </span>
                    </button>
                  </div>
                </ContactActionsContextMenu>
              );
            })}
            {contactsQuery.hasNextPage && !autoLoadMoreContacts ? (
              <Button
                variant="ghost"
                className="mx-4 my-3"
                loading={contactsQuery.isFetchingNextPage}
                onClick={() => void contactsQuery.fetchNextPage()}
              >
                {t("contacts.loadMore")}
              </Button>
            ) : contactsQuery.isFetchingNextPage ? (
              <span className="mx-auto my-3"><Spinner size={18} /></span>
            ) : null}
          </OverlayScrollArea>
        )}
      </Page>

      <ResizeHandle
        value={listPaneWidth}
        min={310}
        max={listPaneMax}
        onValueChange={onListPaneWidthChange}
        label={t("contacts.resizeListPane")}
      />

      <Page className="flex min-h-0 flex-col bg-card">
        {!selectedContactId ? (
          <EmptyState className="m-auto" icon={<UserRound size={21} />} title={t("contacts.selectContact")} />
        ) : detailQuery.isPending ? (
          <EmptyState className="m-auto" icon={<UserRound size={21} />} title={t("contacts.loadingDetail")} />
        ) : detailQuery.isError || !detailQuery.data ? (
          <EmptyState
            className="m-auto"
            icon={<UserRound size={21} />}
            title={t("contacts.loadDetailFailed")}
            action={<Button variant="secondary" onClick={() => void detailQuery.refetch()}>{t("common.retry")}</Button>}
          />
        ) : (
          <ContactDetailView
            detail={detailQuery.data}
            composing={composeMutation.isPending}
            onCompose={() => composeMutation.mutate(detailQuery.data.contact.id)}
            onEdit={() => {
              setOperationError(null);
              setEditor({ mode: "edit", contact: detailQuery.data.contact });
            }}
            onNavigate={(mailboxId, messageId) => onNavigateToMessage({
              accountId,
              mailboxId,
              messageId,
            })}
          />
        )}
      </Page>

      {operationError ? (
        <Alert className="fixed right-4 bottom-4 z-40 max-w-sm bg-popover shadow-xl" tone="danger">
          {t(`errors.${operationError}`, { defaultValue: t("common.unexpectedError") })}
        </Alert>
      ) : null}
      <ContactEditor
        state={editor}
        busy={createMutation.isPending || updateMutation.isPending || deleteMutation.isPending}
        errorCode={operationError}
        onClose={() => setEditor(null)}
        onCreate={(draft) => createMutation.mutate(draft)}
        onUpdate={(contact, name) => updateMutation.mutate({ contact, name })}
      />
    </Page>
  );
}

function ContactActionsContextMenu({
  contact,
  selectionCount,
  pending,
  onCompose,
  onEdit,
  onDelete,
  onCopyError,
  children,
}: {
  contact: ContactSummary;
  selectionCount: number;
  pending: boolean;
  onCompose: () => void;
  onEdit: () => void;
  onDelete: () => void;
  onCopyError: () => void;
  children: ReactElement;
}) {
  const { t } = useTranslation();
  const single = selectionCount === 1;
  const copy = (value: string) => void writeClipboardText(value).catch(onCopyError);
  return (
    <ContextMenu>
      <ContextMenuTrigger asChild>{children}</ContextMenuTrigger>
      <ContextMenuContent>
        {single ? (
          <>
            <ContextMenuItem disabled={pending} onSelect={() => copy(contact.name)}><UserRound size={16} />{t("contacts.copyName")}</ContextMenuItem>
            <ContextMenuItem disabled={pending} onSelect={() => copy(contact.email)}><Mail size={16} />{t("contacts.copyEmail")}</ContextMenuItem>
            <ContextMenuItem disabled={pending} onSelect={() => copy(`${contact.name} <${contact.email}>`)}><Copy size={16} />{t("contacts.copyFullAddress")}</ContextMenuItem>
            <ContextMenuSeparator />
            <ContextMenuItem disabled={pending} onSelect={onCompose}><Send size={16} />{t("contacts.sendMail")}</ContextMenuItem>
            <ContextMenuItem disabled={pending} onSelect={() => window.setTimeout(onEdit, 0)}><Pencil size={16} />{t("contacts.edit")}</ContextMenuItem>
            <ContextMenuSeparator />
          </>
        ) : null}
        <ContextMenuItem
          className="text-destructive focus:bg-destructive/10 focus:text-destructive"
          disabled={pending}
          onSelect={() => window.setTimeout(onDelete, 0)}
        >
          <Trash2 size={16} />
          {selectionCount > 1 ? t("contacts.deleteSelected", { count: selectionCount }) : t("common.delete")}
        </ContextMenuItem>
      </ContextMenuContent>
    </ContextMenu>
  );
}

function ContactDetailView({
  detail,
  composing,
  onCompose,
  onEdit,
  onNavigate,
}: {
  detail: Awaited<ReturnType<typeof api.getContactDetail>>;
  composing: boolean;
  onCompose: () => void;
  onEdit: () => void;
  onNavigate: (mailboxId: string, messageId: string) => void;
}) {
  const { t, i18n } = useTranslation();
  const createdAt = new Intl.DateTimeFormat(i18n.language, { dateStyle: "medium" })
    .format(new Date(detail.contact.createdAt * 1000));
  const updatedAt = new Intl.DateTimeFormat(i18n.language, { dateStyle: "medium" })
    .format(new Date(detail.contact.updatedAt * 1000));
  return (
    <OverlayScrollArea className="min-h-0 flex-1" viewportClassName="px-8 py-8">
      <Stack className="mx-auto w-full max-w-4xl" gap="lg">
        <Inline className="flex-wrap justify-between gap-5">
          <Inline className="min-w-0 gap-4">
            <ContactInitial name={detail.contact.name} className="size-20 text-2xl" />
            <Stack gap="xs">
              <Heading level={1}>{detail.contact.name}</Heading>
              <ContactIdentity address={{
                contactId: detail.contact.id,
                name: detail.contact.name,
                headerName: null,
                email: detail.contact.email,
              }} onEditContact={() => onEdit()} tag>
                <span className="inline-flex items-center gap-2.5 text-muted-foreground">
                  <Mail size={15} /><span className="select-text text-sm">{detail.contact.email}</span>
                </span>
              </ContactIdentity>
              <Inline className="text-muted-foreground"><Clock3 size={15} /><Text>{t("contacts.createdAt", { date: createdAt })}</Text></Inline>
              <Inline className="text-muted-foreground"><Pencil size={15} /><Text>{t("contacts.updatedAt", { date: updatedAt })}</Text></Inline>
            </Stack>
          </Inline>
          <Inline className="flex-wrap">
            <Button loading={composing} onClick={onCompose}><Send size={16} />{t("contacts.sendMail")}</Button>
            <Button variant="secondary" onClick={onEdit}><Pencil size={16} />{t("contacts.edit")}</Button>
          </Inline>
        </Inline>
        <div className="h-px bg-border" />
        <Stack gap="sm">
          <Heading level={2}>{t("contacts.recentMessages")}</Heading>
          {detail.recentMessages.length ? (
            <div className="overflow-hidden rounded-lg border border-border">
              {detail.recentMessages.map((message) => (
                <button
                  key={message.messageId}
                  type="button"
                  className="flex w-full cursor-pointer items-center gap-3 border-b border-border px-4 py-3 text-left outline-none last:border-b-0 hover:bg-muted/70 focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-ring/60"
                  onClick={() => onNavigate(message.mailboxId, message.messageId)}
                >
                  <Mail size={17} className="shrink-0 text-primary" />
                  <span className="min-w-0 flex-1 truncate text-sm font-medium text-foreground">
                    {message.subject || t("mail.noSubject")}
                  </span>
                  <span className="shrink-0 text-xs text-muted-foreground">
                    {new Intl.DateTimeFormat(i18n.language, { dateStyle: "medium" })
                      .format(new Date(message.receivedAt * 1000))}
                  </span>
                </button>
              ))}
            </div>
          ) : (
            <Text>{t("contacts.noRecentMessages")}</Text>
          )}
        </Stack>
      </Stack>
    </OverlayScrollArea>
  );
}

function ContactEditor({
  state,
  busy,
  errorCode,
  onClose,
  onCreate,
  onUpdate,
}: {
  state: EditorState;
  busy: boolean;
  errorCode: string | null;
  onClose: () => void;
  onCreate: (draft: ContactDraft) => void;
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
          if (state.mode === "create") onCreate({ name, email });
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
          <Alert tone="danger">{t(`errors.${errorCode}`, { defaultValue: t("common.unexpectedError") })}</Alert>
        ) : null}
        <Inline className="justify-end pt-2">
          <Button type="button" variant="ghost" disabled={busy} onClick={onClose}>{t("common.cancel")}</Button>
          <Button type="submit" loading={busy} disabled={!name.trim() || (state.mode === "create" && !email.trim())}>
            {t("common.save")}
          </Button>
        </Inline>
      </form>
    </Modal>
  );
}
