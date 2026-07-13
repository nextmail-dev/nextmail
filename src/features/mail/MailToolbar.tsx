import { Copy, Download, Forward, Info, LogOut, MoreHorizontal, Reply, ReplyAll, Settings, Users } from "lucide-react";
import { useTranslation } from "react-i18next";

import { Button } from "@/components/ui/button";
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuSeparator, DropdownMenuTrigger } from "@/components/ui/dropdown-menu";
import { Inline } from "@/components/ui/layout";
import { SearchField } from "@/components/ui/search-field";
import type { MailboxSummary, MessageComposeAction } from "@/app/types";

interface MailToolbarProps {
  onOpenSettings: () => void;
  onOpenAccounts: () => void;
  onOpenAbout: () => void;
  onQuit: () => void;
  searchQuery: string;
  onSearchChange: (value: string) => void;
  onReceive: () => void;
  receiving: boolean;
  selectedMessageId: string;
  selectedMailboxId: string;
  mailboxes: MailboxSummary[];
  activeMessageAction: MessageComposeAction | "copy" | null;
  onMessageAction: (action: MessageComposeAction) => void;
  onCopy: (destinationMailboxId: string) => void;
}

export function MailToolbar({
  onOpenSettings,
  onOpenAccounts,
  onOpenAbout,
  onQuit,
  searchQuery,
  onSearchChange,
  onReceive,
  receiving,
  selectedMessageId,
  selectedMailboxId,
  mailboxes,
  activeMessageAction,
  onMessageAction,
  onCopy,
}: MailToolbarProps) {
  const { t } = useTranslation();
  const destinations = mailboxes.filter((mailbox) => mailbox.selectable && mailbox.id !== selectedMailboxId);
  return (
    <Inline className="h-full overflow-x-auto bg-card px-3 shadow-[inset_0_-1px_0_var(--border)]" role="toolbar" aria-label={t("mail.mainToolbar")}>
      <Button variant="ghost" loading={receiving} onClick={onReceive}><Download size={17} />{t("mail.receive")}</Button>
      <Button variant="ghost" disabled={!selectedMessageId} loading={activeMessageAction === "reply"} onClick={() => onMessageAction("reply")}>
        <Reply size={16} />{t("mail.reply")}
      </Button>
      <Button variant="ghost" disabled={!selectedMessageId} loading={activeMessageAction === "reply_all"} onClick={() => onMessageAction("reply_all")}>
        <ReplyAll size={16} />{t("mail.replyAll")}
      </Button>
      <Button variant="ghost" disabled={!selectedMessageId} loading={activeMessageAction === "forward"} onClick={() => onMessageAction("forward")}>
        <Forward size={16} />{t("mail.forward")}
      </Button>
      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <Button variant="ghost" disabled={!selectedMessageId || !destinations.length} loading={activeMessageAction === "copy"}>
            <Copy size={16} />{t("mail.copyTo")}
          </Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent align="start" className="max-h-72 overflow-auto">
          {destinations.map((mailbox) => (
            <DropdownMenuItem key={mailbox.id} onSelect={() => onCopy(mailbox.id)}>
              {mailbox.role === "other" ? mailbox.name : t(`mailboxNames.${mailbox.role}`)}
            </DropdownMenuItem>
          ))}
        </DropdownMenuContent>
      </DropdownMenu>
      <Inline className="ml-auto">
        <SearchField value={searchQuery} onValueChange={onSearchChange} placeholder={t("mail.searchCurrentFolder")} aria-label={t("mail.searchCurrentFolder")} clearLabel={t("mail.clearSearch")} />
        <DropdownMenu>
          <DropdownMenuTrigger asChild><Button variant="ghost" size="icon" aria-label={t("mail.appMenu")}><MoreHorizontal size={19} /></Button></DropdownMenuTrigger>
          <DropdownMenuContent align="end">
            <DropdownMenuItem onSelect={onOpenSettings}><Settings size={16} />{t("mail.settings")}</DropdownMenuItem>
            <DropdownMenuItem onSelect={onOpenAccounts}><Users size={16} />{t("mail.accountManagement")}</DropdownMenuItem>
            <DropdownMenuSeparator />
            <DropdownMenuItem onSelect={onOpenAbout}><Info size={16} />{t("mail.about")}</DropdownMenuItem>
            <DropdownMenuItem onSelect={onQuit}><LogOut size={16} />{t("mail.quit")}</DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
      </Inline>
    </Inline>
  );
}
