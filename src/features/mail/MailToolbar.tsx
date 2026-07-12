import {
  ChevronDown,
  Download,
  Info,
  FilePenLine,
  LogOut,
  MailPlus,
  MoreHorizontal,
  Settings,
  Users,
} from "lucide-react";
import { useTranslation } from "react-i18next";

import type { AccountSummary, DraftListItem } from "@/app/types";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuCheckboxItem,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Inline, Stack } from "@/components/ui/layout";
import { Text } from "@/components/ui/typography";

interface MailToolbarProps {
  accounts: AccountSummary[];
  selectedAccountId: string;
  onAccountChange: (accountId: string) => void;
  onOpenSettings: () => void;
  onCompose: () => void;
  drafts: DraftListItem[];
  onOpenDraft: (draftId: string) => void;
  onOpenAccounts: () => void;
  onOpenAbout: () => void;
  onQuit: () => void;
}

export function MailToolbar({
  accounts,
  selectedAccountId,
  onAccountChange,
  onOpenSettings,
  onCompose,
  drafts,
  onOpenDraft,
  onOpenAccounts,
  onOpenAbout,
  onQuit,
}: MailToolbarProps) {
  const { t } = useTranslation();
  const selected = accounts.find((account) => account.id === selectedAccountId) ?? accounts[0];

  return (
    <Inline className="h-14 border-b border-border bg-card px-3 shadow-xs">
      <Inline className="w-[15rem] shrink-0">
        {accounts.length > 1 ? (
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <Button
                variant="ghost"
                className="h-10 w-full justify-between px-2.5 text-left"
                aria-label={t("mail.switchAccount")}
              >
                <AccountIdentity account={selected} />
                <ChevronDown size={15} />
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent className="w-64" align="start">
              <DropdownMenuLabel>{t("mail.accounts")}</DropdownMenuLabel>
              {accounts.map((account) => (
                <DropdownMenuCheckboxItem
                  key={account.id}
                  checked={account.id === selected?.id}
                  onCheckedChange={() => onAccountChange(account.id)}
                >
                  <AccountIdentity account={account} />
                </DropdownMenuCheckboxItem>
              ))}
            </DropdownMenuContent>
          </DropdownMenu>
        ) : (
          <AccountIdentity account={selected} />
        )}
      </Inline>

      <Inline className="flex-1 gap-1 border-l border-border pl-3">
        <Button variant="ghost" disabled aria-label={t("mail.receiveDisabledHint")}>
          <Download size={17} />
          {t("mail.receive")}
        </Button>
        <Button variant="ghost" onClick={onCompose} aria-label={t("mail.compose")}>
          <MailPlus size={17} />
          {t("mail.compose")}
        </Button>
        {drafts.length ? (
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <Button variant="ghost" size="icon" aria-label={t("composer.openDraft")}>
                <ChevronDown size={15} />
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="start" className="w-72">
              <DropdownMenuLabel>{t("composer.localDrafts")}</DropdownMenuLabel>
              {drafts.map((draft) => (
                <DropdownMenuItem key={draft.id} onSelect={() => onOpenDraft(draft.id)}>
                  <FilePenLine size={15} />
                  <Stack gap="xs" className="min-w-0">
                    <Text className="truncate text-[13px] text-foreground">
                      {draft.subject || t("mail.noSubject")}
                    </Text>
                    <Text className="truncate text-[11px]">
                      {draft.recipients.map((recipient) => recipient.name || recipient.email).join(", ") || t("composer.noRecipients")}
                    </Text>
                  </Stack>
                </DropdownMenuItem>
              ))}
            </DropdownMenuContent>
          </DropdownMenu>
        ) : null}
      </Inline>

      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <Button variant="ghost" size="icon" aria-label={t("mail.appMenu")}>
            <MoreHorizontal size={19} />
          </Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent align="end">
          <DropdownMenuItem onSelect={onOpenSettings}>
            <Settings size={16} />
            {t("mail.settings")}
          </DropdownMenuItem>
          <DropdownMenuItem onSelect={onOpenAccounts}>
            <Users size={16} />
            {t("mail.accountManagement")}
          </DropdownMenuItem>
          <DropdownMenuSeparator />
          <DropdownMenuItem onSelect={onOpenAbout}>
            <Info size={16} />
            {t("mail.about")}
          </DropdownMenuItem>
          <DropdownMenuItem onSelect={onQuit}>
            <LogOut size={16} />
            {t("mail.quit")}
          </DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>
    </Inline>
  );
}

function AccountIdentity({ account }: { account?: AccountSummary }) {
  if (!account) return null;
  return (
    <Stack className="min-w-0 flex-1" gap="xs">
      <Text className="truncate text-[13px] font-semibold leading-none text-foreground">
        {account.displayName || account.email}
      </Text>
      {account.displayName ? (
        <Text className="truncate text-[11px] leading-none">{account.email}</Text>
      ) : null}
    </Stack>
  );
}
