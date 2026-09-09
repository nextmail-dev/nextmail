import { invoke as nativeInvoke } from "@tauri-apps/api/core";
import type { AppearancePreferences, AddressPresentation, ContactSummary, LanguagePreference, MailboxRole, MailboxSummary, MessageDetail, MessageListItem } from "../types";

export interface DemoLetter {
  name: string;
  email: string;
  subject: string;
  text: string;
  html: string | null;
  attachment?: string;
  priority?: boolean;
  folder?: MailboxRole;
  remoteImagesBlocked?: boolean;
}
export type DemoContent = Record<LanguagePreference, DemoLetter[]>;
type Args = Record<string, unknown>;

let store: DemoStore | null = null;
export const isDemoMode = () => store !== null;

export async function initializeDemoSession() {
  if (!("__TAURI_INTERNALS__" in globalThis)) return;
  const preferences = await nativeInvoke<AppearancePreferences | null>("get_demo_status");
  if (preferences) {
    const content = await nativeInvoke<DemoContent>("get_demo_content");
    store = new DemoStore(content, preferences.language);
  }
}

// All application IPC goes through this gate. The Rust gate separately denies
// real commands, including calls left over from a window being disposed.
export async function invoke<T>(command: string, args?: Args): Promise<T> {
  if (!store) return nativeInvoke<T>(command, args);
  if (["get_preferences", "set_appearance_preferences", "get_app_about", "quit_app", "resolve_main_close"].includes(command)) {
    const result = await nativeInvoke<T>(command, args);
    if (command === "get_preferences" || command === "set_appearance_preferences") {
      store.language = (result as AppearancePreferences).language;
    }
    return result;
  }
  return structuredClone(store.dispatch(command, args ?? {})) as T;
}

const unavailable = () => ({ code: "demo.unavailable", params: {}, retryable: false });
const accountId = "demo-account";
const roles: MailboxRole[] = ["inbox", "sent", "drafts", "archive", "junk", "trash", "other"];
const folderNames = {
  "zh-CN": ["收件箱", "已发送", "草稿", "归档", "垃圾邮件", "已删除", "项目资料"],
  "en-US": ["Inbox", "Sent", "Drafts", "Archive", "Junk", "Trash", "Projects"],
};
const senderNames: Record<LanguagePreference, string[]> = {
  "zh-CN": [
    "林知夏 · NextMail 产品设计",
    "NextMail 商务简报",
    "NextMail 商务服务",
    "NextMail 财务中心",
    "NextMail 商务差旅",
    "陈予安 · NextMail 产品团队",
    "NextMail 活动中心",
    "NextMail 工程协作",
    "NextMail 品牌中心",
    "林知夏 · NextMail 项目组",
    "NextMail 项目办公室",
    "NextMail 商务资讯",
  ],
  "en-US": [
    "Maya Lin · NextMail Product Design",
    "NextMail Business Briefing",
    "NextMail Business Services",
    "NextMail Finance",
    "NextMail Business Travel",
    "Alex Chen · NextMail Product",
    "NextMail Events",
    "NextMail Engineering",
    "NextMail Brand Studio",
    "Maya Lin · NextMail Projects",
    "NextMail Project Office",
    "NextMail Business Updates",
  ],
};

/** An isolated, in-memory read model. Never falls back to a real command. */
export class DemoStore {
  private readonly receivedBase = Math.floor(Date.now() / 1000);
  private messages: Array<{ id: string; index: number; folder: string; unread: boolean; flagged: boolean }>;
  private folders: Array<{ id: string; role: MailboxRole; favorite: boolean; name?: string }>;
  private lastMailbox = "demo-inbox";
  constructor(private content: DemoContent, public language: LanguagePreference) {
    this.messages = content[language].map((letter, index) => ({
      id: `demo-message-${index}`, index, folder: `demo-${letter.folder ?? "inbox"}`,
      unread: index < 5, flagged: [0, 1, 4].includes(index),
    }));
    this.folders = roles.map((role) => ({ id: `demo-${role}`, role, favorite: role === "inbox" }));
  }

  private account() {
    return { id: accountId, email: "business@next-mail.app", displayName: this.language === "zh-CN" ? "NextMail 商务团队" : "NextMail Business", lastSelectedMailboxId: this.lastMailbox };
  }

  private address(letter: DemoLetter, index: number): AddressPresentation {
    const localPart = letter.email.split("@", 1)[0];
    const name = senderNames[this.language][index] ?? letter.name;
    return { name, headerName: name, email: `${localPart}@next-mail.app`, contactId: `demo-contact-${index}` };
  }

  private details(): MessageDetail[] {
    return this.messages.map((message) => {
      const letter = this.content[this.language][message.index];
      return {
        id: message.id, mailboxId: message.folder, subject: letter.subject,
        from: [this.address(letter, message.index)],
        to: [{ name: this.account().displayName, headerName: null, email: this.account().email, contactId: null }], cc: [],
        receivedAt: this.receivedBase - message.index * 4500, plainText: letter.text, safeHtml: letter.html,
        bodyAvailability: "available", attachments: letter.attachment ? [{ id: `demo-attachment-${message.index}`, fileName: letter.attachment, contentType: letter.attachment.endsWith(".pdf") ? "application/pdf" : "text/plain", size: 245760 + message.index * 1024, availability: "missing" }] : [],
        remoteImagesBlocked: letter.remoteImagesBlocked ?? false, revision: 1,
        unread: message.unread, flagged: message.flagged, highPriority: letter.priority ?? false, pendingOperation: false,
      };
    });
  }

  private contacts(): ContactSummary[] {
    return this.content[this.language].slice(0, 9).map((letter, index) => ({
      id: `demo-contact-${index}`, name: senderNames[this.language][index] ?? letter.name, email: `${letter.email.split("@", 1)[0]}@next-mail.app`, revision: 1, createdAt: 1788912000, updatedAt: 1788912000,
    }));
  }

  dispatch(command: string, args: Args): unknown {
    if (args.accountId != null && args.accountId !== accountId) throw unavailable();
    const details = () => this.details();
    const selection = () => this.messages.filter((message) =>
      (args.messageIds as string[] | undefined)?.includes(message.id)
      && message.folder === (args.mailboxId ?? args.sourceMailboxId));
    switch (command) {
      case "get_bootstrap_status": return { stage: "ready", defaultDataDir: "", configuredDataDir: null, accounts: [this.account()], lastSelectedAccountId: accountId };
      case "list_account_summaries": return [this.account()];
      case "get_last_selected_account": case "set_last_selected_account": return accountId;
      case "set_last_selected_mailbox": this.lastMailbox = String(args.mailboxId); return this.lastMailbox;
      case "list_account_runtime_summaries": return [{ accountId, state: "ready", errorCode: null, retryAt: null, revision: 1 }];
      case "get_reading_preferences": return { autoLoadRemoteImages: false, autoLoadMoreMessages: true, autoLoadMoreContacts: true };
      case "get_desktop_preferences": return { minimizeToTray: false, askBeforeExit: true, autoCheckUpdates: false };
      case "get_sync_progress": return { accountId, phase: "complete", completed: this.messages.length, total: this.messages.length, currentMailboxName: null, errorCode: null, revision: 1 };
      case "start_background_services": case "sync_now": case "log_frontend_event": return;
      case "open_composer": return "";
      case "list_pending_operation_status": case "list_drafts": return [];
      case "list_mailboxes": return this.folders.map((folder): MailboxSummary => ({
        id: folder.id, accountId, name: folder.name ?? folderNames[this.language][roles.indexOf(folder.role)], delimiter: "/", role: folder.role, selectable: true,
        totalCount: this.messages.filter((m) => m.folder === folder.id).length,
        unreadCount: this.messages.filter((m) => m.folder === folder.id && m.unread).length,
        isFavorite: folder.favorite, revision: 1,
      }));
      case "list_messages": case "list_unread_messages": case "list_starred_messages": case "search_messages": {
        const query = String(args.query ?? "").toLocaleLowerCase();
        let items = details().filter((message) => {
          if (command === "list_unread_messages") return message.unread;
          if (command === "list_starred_messages") return message.flagged;
          if (args.mailboxId && message.mailboxId !== args.mailboxId) return false;
          return command !== "search_messages" || [message.subject, message.plainText, ...message.from.flatMap((a) => [a.name, a.email]), ...message.to.flatMap((a) => [a.name, a.email])].join(" ").toLocaleLowerCase().includes(query);
        }).map((message): MessageListItem => ({
          id: message.id, mailboxId: message.mailboxId, subject: message.subject, from: message.from, receivedAt: message.receivedAt,
          preview: message.plainText ?? "", unread: message.unread, flagged: message.flagged, highPriority: message.highPriority,
          hasAttachments: message.attachments.length > 0, bodyAvailability: "available", pendingOperation: false,
        }));
        const offset = Number(args.cursor ?? 0);
        const limit = Number(args.limit ?? 50);
        const more = offset + limit < items.length;
        items = items.slice(offset, offset + limit);
        return { items, nextCursor: more ? String(offset + limit) : null };
      }
      case "get_message_detail": case "request_message_body": {
        const message = details().find((item) => item.id === args.messageId && item.mailboxId === args.mailboxId);
        if (!message) throw unavailable();
        return message;
      }
      case "set_message_read": selection().forEach((message) => { message.unread = !args.read; }); return;
      case "set_message_flagged": selection().forEach((message) => { message.flagged = Boolean(args.flagged); }); return;
      case "mark_mailbox_all_read": this.messages.filter((message) => message.folder === args.mailboxId).forEach((message) => { message.unread = false; }); return;
      case "move_messages": case "archive_messages": case "delete_messages": case "copy_messages": {
        const destination = command === "archive_messages" ? "demo-archive" : command === "delete_messages" ? "demo-trash" : String(args.destinationMailboxId);
        if (!this.folders.some((folder) => folder.id === destination)) throw unavailable();
        const selected = selection();
        if (command === "copy_messages") this.messages.push(...selected.map((message) => ({ ...message, id: crypto.randomUUID(), folder: destination })));
        else if (command === "delete_messages" && args.sourceMailboxId === "demo-trash") this.messages = this.messages.filter((message) => !selected.includes(message));
        else selected.forEach((message) => { message.folder = destination; });
        return;
      }
      case "set_mailbox_favorite": {
        const folder = this.folders.find((item) => item.id === args.mailboxId);
        if (folder) folder.favorite = Boolean(args.favorite);
        return;
      }
      case "create_mailbox": this.folders.push({ id: `demo-folder-${crypto.randomUUID()}`, role: "other", favorite: false, name: String(args.name) }); return;
      case "rename_mailbox": {
        const folder = this.folders.find((item) => item.id === args.mailboxId);
        if (folder) folder.name = String(args.name);
        return;
      }
      case "delete_mailbox": this.folders = this.folders.filter((folder) => folder.id !== args.mailboxId); this.messages = this.messages.filter((message) => message.folder !== args.mailboxId); return;
      case "reorder_mailboxes": this.folders.sort((a, b) => (args.orderedMailboxIds as string[]).indexOf(a.id) - (args.orderedMailboxIds as string[]).indexOf(b.id)); return;
      case "list_contacts": {
        const query = String(args.query ?? "").toLocaleLowerCase();
        const contacts = this.contacts().filter((contact) => `${contact.name} ${contact.email}`.toLocaleLowerCase().includes(query));
        return { items: contacts, nextCursor: null, total: contacts.length };
      }
      case "get_contact_summary": case "get_contact_detail": {
        const contact = this.contacts().find((item) => item.id === args.contactId);
        if (!contact) throw unavailable();
        return command === "get_contact_summary" ? contact : { contact, recentMessages: details().filter((m) => m.from.some((a) => a.email === contact.email)).map((m) => ({ messageId: m.id, mailboxId: m.mailboxId, subject: m.subject, receivedAt: m.receivedAt })) };
      }
      case "resolve_contact_addresses": return (args.addresses as AddressPresentation[]).map((a) => ({ ...a, headerName: a.name, contactId: this.contacts().find((c) => c.email === a.email)?.id ?? null }));
      case "list_contact_groups": return [{ id: "demo-group", name: this.language === "zh-CN" ? "创作伙伴" : "Creative partners", memberCount: 2, revision: 1 }];
      case "get_contact_group": return { group: (this.dispatch("list_contact_groups", {}) as unknown[])[0], members: [this.contacts()[0], this.contacts()[5]] };
      default: throw unavailable();
    }
  }
}
