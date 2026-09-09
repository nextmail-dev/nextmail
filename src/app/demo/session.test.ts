import { describe, expect, it, vi } from "vitest";
import { invoke as nativeInvoke } from "@tauri-apps/api/core";
import content from "./messages.json";
import { DemoStore, initializeDemoSession, invoke, type DemoContent } from "./session";
import type { MessageDetail, MessageListPage } from "../types";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
const seed = content as DemoContent;
const args = { accountId: "demo-account", mailboxId: "demo-inbox" };

describe("isolated demo session", () => {
  it("supports paging, global search and local read/star/move changes", () => {
    const store = new DemoStore(seed, "en-US");
    const page = store.dispatch("list_messages", { ...args, limit: 2 }) as MessageListPage;
    expect(page.items).toHaveLength(2);
    expect(page.nextCursor).toBe("2");
    expect((store.dispatch("list_messages", { ...args, cursor: page.nextCursor, limit: 2 }) as MessageListPage).items[0].id).not.toBe(page.items[0].id);
    const id = page.items[0].id;
    store.dispatch("set_message_read", { ...args, messageIds: [id], read: true });
    store.dispatch("set_message_flagged", { ...args, messageIds: [id], flagged: false });
    expect(store.dispatch("get_message_detail", { ...args, messageId: id })).toMatchObject({ unread: false, flagged: false });
    store.dispatch("move_messages", { accountId: args.accountId, sourceMailboxId: args.mailboxId, destinationMailboxId: "demo-archive", messageIds: [id] });
    expect((store.dispatch("search_messages", { accountId: args.accountId, mailboxId: null, query: "Autumn" }) as MessageListPage).items[0].mailboxId).toBe("demo-archive");
    expect((store.dispatch("search_messages", { ...args, query: "Autumn" }) as MessageListPage).items).toHaveLength(0);
  });

  it("changes all localized content while retaining local message state", () => {
    const store = new DemoStore(seed, "zh-CN");
    store.dispatch("set_message_read", { ...args, messageIds: ["demo-message-0"], read: true });
    store.language = "en-US";
    const detail = store.dispatch("get_message_detail", { ...args, messageId: "demo-message-0" }) as MessageDetail;
    expect(detail.subject).toContain("Autumn");
    expect(detail.from[0].name).toBe("Maya Lin · NextMail Product Design");
    expect(detail.from[0].email).toBe("lin@next-mail.app");
    expect(detail.attachments[0].fileName).toBe("Autumn brand proposal.pdf");
    expect(detail.unread).toBe(false);
    expect(new DemoStore(seed, "en-US").dispatch("get_message_detail", { ...args, messageId: "demo-message-0" })).toMatchObject({ unread: true });
  });

  it("uses the product domain throughout the visible demo identities", () => {
    for (const language of ["zh-CN", "en-US"] as const) {
      const store = new DemoStore(seed, language);
      const account = (store.dispatch("get_bootstrap_status", {}) as { accounts: Array<{ email: string }> }).accounts[0];
      const messages = store.dispatch("list_messages", { ...args, limit: 50 }) as MessageListPage;
      const contacts = store.dispatch("list_contacts", { accountId: "demo-account", query: "", limit: 50 }) as { items: Array<{ email: string }> };
      expect(account.email).toBe("business@next-mail.app");
      expect(messages.items.every((message) => message.from.every((address) => address.email.endsWith("@next-mail.app")))).toBe(true);
      expect(contacts.items.every((contact) => contact.email.endsWith("@next-mail.app"))).toBe(true);
    }
  });

  it("rejects unknown commands, real IDs and every file/account/send action without native fallback", async () => {
    Object.defineProperty(globalThis, "__TAURI_INTERNALS__", { configurable: true, value: {} });
    vi.mocked(nativeInvoke).mockResolvedValueOnce({ theme: "light", accentColor: "#2563eb", language: "en-US" }).mockResolvedValueOnce(seed);
    await initializeDemoSession();
    vi.mocked(nativeInvoke).mockClear();
    for (const command of ["open_account_management_window", "queue_draft_send", "open_message_attachment", "save_message_as", "get_account_connection_draft", "future_command"]) {
      await expect(invoke(command)).rejects.toMatchObject({ code: "demo.unavailable" });
    }
    await expect(invoke("list_messages", { accountId: "real-account" })).rejects.toMatchObject({ code: "demo.unavailable" });
    await expect(invoke("get_bootstrap_status")).resolves.toMatchObject({ accounts: [{ id: "demo-account" }] });
    expect(nativeInvoke).not.toHaveBeenCalled();
    Reflect.deleteProperty(globalThis, "__TAURI_INTERNALS__");
  });
});
