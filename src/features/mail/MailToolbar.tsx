import { Download, Info, LogOut, MoreHorizontal, Settings, Users } from "lucide-react";
import { useTranslation } from "react-i18next";

import { Button } from "@/components/ui/button";
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuSeparator, DropdownMenuTrigger } from "@/components/ui/dropdown-menu";
import { Inline } from "@/components/ui/layout";
import { SearchField } from "@/components/ui/search-field";

interface MailToolbarProps {
  onOpenSettings: () => void;
  onOpenAccounts: () => void;
  onOpenAbout: () => void;
  onQuit: () => void;
  searchQuery: string;
  onSearchChange: (value: string) => void;
}

export function MailToolbar({ onOpenSettings, onOpenAccounts, onOpenAbout, onQuit, searchQuery, onSearchChange }: MailToolbarProps) {
  const { t } = useTranslation();
  return (
    <Inline className="h-full bg-card px-3 shadow-[inset_0_-1px_0_var(--border)]">
      <Button variant="ghost" disabled aria-label={t("mail.receiveDisabledHint")}><Download size={17} />{t("mail.receive")}</Button>
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
