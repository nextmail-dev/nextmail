import { ChevronDown, UserRound } from "lucide-react";
import { useTranslation } from "react-i18next";

import type { AccountSummary } from "@/app/types";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuCheckboxItem,
  DropdownMenuContent,
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

export function AccountSwitcher({
  accounts,
  selectedAccountId,
  onAccountChange,
  collapsed = false,
}: AccountSwitcherProps) {
  const { t } = useTranslation();
  const selected = accounts.find((account) => account.id === selectedAccountId) ?? accounts[0];
  const identity = <AccountIdentity account={selected} collapsed={collapsed} />;

  return (
    <Inline className={collapsed ? "justify-center px-2 pt-5" : "px-4 pt-5"}>
      {accounts.length > 1 ? (
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <Button
              variant="ghost"
              className={collapsed ? "size-10 p-0" : "h-12 min-w-0 flex-1 justify-start px-1"}
              aria-label={t("mail.switchAccount")}
            >
              {identity}
              {collapsed ? null : <ChevronDown className="ml-auto" size={15} />}
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent className="w-72" align="start">
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
      ) : identity}
    </Inline>
  );
}

function AccountIdentity({ account, collapsed = false }: { account?: AccountSummary; collapsed?: boolean }) {
  if (!account) return null;
  const label = account.displayName || account.email;
  const initial = label.trim().charAt(0).toLocaleUpperCase() || <UserRound size={16} />;
  return (
    <Inline className={collapsed ? "min-w-0 justify-center" : "min-w-0 flex-1"} title={collapsed ? account.email : undefined}>
      <span className="grid size-10 shrink-0 place-items-center rounded-full bg-primary text-sm font-bold text-primary-foreground shadow-[0_7px_18px_color-mix(in_srgb,var(--primary)_22%,transparent)]">
        {initial}
      </span>
      {collapsed ? null : (
        <Stack className="min-w-0 flex-1" gap="xs">
          <Text className="truncate text-sm font-semibold leading-none text-foreground">{label}</Text>
          <Text className="truncate text-[11px] leading-none">{account.email}</Text>
        </Stack>
      )}
    </Inline>
  );
}
