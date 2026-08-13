import { ChevronDown, Settings, UserRound } from "lucide-react";
import { useTranslation } from "react-i18next";

import type { AccountRuntimeSummary, AccountSummary } from "@/app/types";
import { Button } from "@/components/ui/button";
import { IdentityAvatar } from "@/components/ui/identity-avatar";
import {
  DropdownMenu,
  DropdownMenuCheckboxItem,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Inline, Stack } from "@/components/ui/layout";
import { Text } from "@/components/ui/typography";

interface AccountSwitcherProps {
  accounts: AccountSummary[];
  selectedAccountId: string;
  onAccountChange: (accountId: string) => void;
  onManageAccounts: () => void;
  runtimeSummaries?: AccountRuntimeSummary[];
  collapsed?: boolean;
}

export function AccountSwitcher({
  accounts,
  selectedAccountId,
  onAccountChange,
  onManageAccounts,
  runtimeSummaries = [],
  collapsed = false,
}: AccountSwitcherProps) {
  const { t } = useTranslation();
  const selected = accounts.find((account) => account.id === selectedAccountId) ?? accounts[0];
  const identity = <AccountIdentity account={selected} runtime={runtimeSummaries.find((item) => item.accountId === selected?.id)} collapsed={collapsed} />;

  return (
    <Inline className={collapsed ? "justify-center px-2 pt-4" : "px-4 pt-4"}>
      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <Button
            variant="ghost"
            className={collapsed ? "size-9 p-0" : "h-auto min-h-10 w-full min-w-0 flex-1 justify-start px-1 py-1"}
            aria-label={t("mail.accountMenu")}
          >
            {identity}
            {collapsed ? null : <ChevronDown className="ml-auto shrink-0" size={15} />}
          </Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent className="w-72" align="start">
          {accounts.map((account) => (
            <DropdownMenuCheckboxItem
              key={account.id}
              className="h-auto min-h-12 py-1.5 pr-3"
              checked={account.id === selected?.id}
              onCheckedChange={() => onAccountChange(account.id)}
            >
              <AccountIdentity account={account} runtime={runtimeSummaries.find((item) => item.accountId === account.id)} />
            </DropdownMenuCheckboxItem>
          ))}
          <DropdownMenuSeparator />
          <DropdownMenuItem onSelect={onManageAccounts}>
            <Settings size={15} />
            {t("mail.accountManagement")}
          </DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>
    </Inline>
  );
}

function AccountIdentity({ account, runtime, collapsed = false }: { account?: AccountSummary; runtime?: AccountRuntimeSummary; collapsed?: boolean }) {
  const { t } = useTranslation();
  if (!account) return null;
  const label = account.displayName || account.email;
  return (
    <Inline className={collapsed ? "min-w-0 justify-center" : "w-full min-w-0 flex-1 justify-start text-left"} title={collapsed ? account.email : undefined}>
      <IdentityAvatar label={label} fallback={<UserRound size={16} />} className="shadow-[var(--shadow-primary)]" />
      {collapsed ? null : (
        <Stack className="min-w-0 flex-1 items-start overflow-hidden whitespace-normal text-left" gap="none">
          <Text className="w-full break-words text-left text-[13px] leading-5 font-semibold whitespace-normal text-foreground">{label}</Text>
          <Text className="w-full break-all text-left text-[length:var(--ui-font-caption)] leading-4 whitespace-normal">
            {account.email}{runtime && !["ready", "stopped", "syncing"].includes(runtime.state) ? ` · ${t(`accounts.runtime.${runtime.state}`)}` : ""}
          </Text>
        </Stack>
      )}
    </Inline>
  );
}
