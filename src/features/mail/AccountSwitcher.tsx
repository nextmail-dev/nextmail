import { ChevronDown, Mail } from "lucide-react";
import { useTranslation } from "react-i18next";

import type { AccountSummary } from "@/app/types";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuCheckboxItem,
  DropdownMenuContent,
  DropdownMenuLabel,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Inline, Stack } from "@/components/ui/layout";
import { Text } from "@/components/ui/typography";

interface AccountSwitcherProps {
  accounts: AccountSummary[];
  selectedAccountId: string;
  onAccountChange: (accountId: string) => void;
  collapsed?: boolean;
}

export function AccountSwitcher({ accounts, selectedAccountId, onAccountChange, collapsed = false }: AccountSwitcherProps) {
  const { t } = useTranslation();
  const selected = accounts.find((account) => account.id === selectedAccountId) ?? accounts[0];
  const identity = <AccountIdentity account={selected} collapsed={collapsed} />;
  if (accounts.length <= 1) {
    return <Inline className={collapsed ? "h-full justify-center px-2" : "h-full px-3"}>{identity}</Inline>;
  }
  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button variant="ghost" className={collapsed ? "h-full w-full rounded-none px-2" : "h-full w-full justify-between rounded-none px-3 text-left"} aria-label={t("mail.switchAccount")}>
          {identity}{collapsed ? null : <ChevronDown size={15} />}
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent className="w-64" align="start">
        <DropdownMenuLabel>{t("mail.accounts")}</DropdownMenuLabel>
        {accounts.map((account) => (
          <DropdownMenuCheckboxItem key={account.id} checked={account.id === selected?.id} onCheckedChange={() => onAccountChange(account.id)}>
            <AccountIdentity account={account} />
          </DropdownMenuCheckboxItem>
        ))}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}

function AccountIdentity({ account, collapsed = false }: { account?: AccountSummary; collapsed?: boolean }) {
  if (!account) return null;
  return (
    <Inline className={collapsed ? "min-w-0 justify-center" : "min-w-0 flex-1"} title={collapsed ? account.email : undefined}>
      <span className="grid size-9 shrink-0 place-items-center rounded-sm bg-foreground text-background"><Mail size={17} /></span>
      {collapsed ? null : <Stack className="min-w-0 flex-1" gap="xs">
        <Text className="truncate text-[13px] font-semibold leading-none text-foreground">{account.displayName || account.email}</Text>
        <Text className="truncate text-[11px] leading-none">{account.email}</Text>
      </Stack>}
    </Inline>
  );
}
