import { ChevronDown, Settings, UserRound } from "lucide-react";
import { useTranslation } from "react-i18next";

import type { AccountRuntimeSummary, AccountSummary } from "@/app/types";
import { Button } from "@/components/ui/button";
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
    <Inline className={collapsed ? "justify-center px-2 pt-5" : "px-4 pt-5"}>
      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <Button
            variant="ghost"
            className={collapsed ? "size-10 p-0" : "h-12 min-w-0 flex-1 justify-start px-1"}
            aria-label={t("mail.accountMenu")}
          >
            {identity}
            {collapsed ? null : <ChevronDown className="ml-auto" size={15} />}
          </Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent className="w-72" align="start">
          {accounts.map((account) => (
            <DropdownMenuCheckboxItem
              key={account.id}
              className="h-auto min-h-14 py-2 pr-3"
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
  const initial = label.trim().charAt(0).toLocaleUpperCase() || <UserRound size={16} />;
  return (
    <Inline className={collapsed ? "min-w-0 justify-center" : "min-w-0 flex-1 justify-start text-left"} title={collapsed ? account.email : undefined}>
      <span className="grid size-10 shrink-0 place-items-center rounded-full bg-primary text-sm font-bold text-primary-foreground shadow-[0_7px_18px_color-mix(in_srgb,var(--primary)_22%,transparent)]">
        {initial}
      </span>
      {collapsed ? null : (
        <Stack className="min-w-0 flex-1 items-start text-left" gap="xs">
          <Text className="w-full truncate text-left text-sm font-semibold leading-none text-foreground">{label}</Text>
          <Text className="w-full truncate text-left text-[length:var(--ui-font-caption)] leading-none">
            {account.email}{runtime && !["ready", "stopped", "syncing"].includes(runtime.state) ? ` · ${t(`accounts.runtime.${runtime.state}`)}` : ""}
          </Text>
        </Stack>
      )}
    </Inline>
  );
}
