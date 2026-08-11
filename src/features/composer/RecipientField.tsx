import { X } from "lucide-react";
import { useQuery } from "@tanstack/react-query";
import { useId, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";

import { api } from "@/app/api";
import type { AddressPresentation, ContactSummary, MessageAddress } from "@/app/types";
import { Button } from "@/components/ui/button";
import { ContactIdentity, ContactInitial } from "@/features/contacts/ContactIdentity";
import { mailQueryKeys } from "@/features/mail/mail-query-keys";
import { cn } from "@/lib/utils";

interface RecipientFieldProps {
  label: string;
  accountId?: string;
  addresses: MessageAddress[];
  resolvedAddresses?: Map<string, AddressPresentation>;
  input: string;
  error?: string | null;
  disabled?: boolean;
  placeholder?: string;
  trailing?: ReactNode;
  structured?: boolean;
  onInputChange: (value: string) => void;
  onCommit: () => void;
  onRemove: (index: number) => void;
  onEditLast: (address: MessageAddress, index: number) => void;
  onSelectContact?: (contact: ContactSummary) => void;
}

export function RecipientField({
  label,
  accountId,
  addresses,
  resolvedAddresses,
  input,
  error,
  disabled,
  placeholder,
  trailing,
  structured = false,
  onInputChange,
  onCommit,
  onRemove,
  onEditLast,
  onSelectContact,
}: RecipientFieldProps) {
  const id = useId();
  const errorId = `${id}-error`;
  const suggestionsId = `${id}-suggestions`;
  const [activeSuggestion, setActiveSuggestion] = useState(-1);
  const trimmedInput = input.trim();
  const suggestions = useQuery({
    queryKey: mailQueryKeys.contactSuggestions(accountId ?? "", trimmedInput),
    queryFn: () => api.listContactSuggestions(accountId ?? "", trimmedInput, 8),
    enabled: Boolean(accountId && trimmedInput && !disabled && onSelectContact),
  });
  const existing = new Set(addresses.map((address) => address.email.trim().toLocaleLowerCase()));
  const visibleSuggestions = (suggestions.data ?? []).filter(
    (contact) => !existing.has(contact.email.trim().toLocaleLowerCase()),
  );
  const activeContact = visibleSuggestions[activeSuggestion];

  function selectSuggestion(contact: ContactSummary) {
    setActiveSuggestion(-1);
    onSelectContact?.(contact);
  }

  return (
    <div className={cn(
      "flex min-h-11 items-start border-b border-border/70 bg-card",
      error && "pb-1",
    )}>
      <label
        htmlFor={id}
        className={cn(
          "shrink-0 px-4 font-semibold text-muted-foreground",
          structured
            ? "flex w-24 self-stretch items-center py-2 text-xs"
            : "w-20 pt-3 text-justify text-sm [text-align-last:justify]",
        )}
      >
        {label}
      </label>
      <div className="relative min-w-0 flex-1 py-1.5">
        <div className="flex min-h-8 flex-wrap items-center gap-1.5">
          {addresses.map((address, index) => (
            <AddressTag
              key={`${address.email.toLocaleLowerCase()}-${index}`}
              address={address}
              presentation={resolvedAddresses?.get(address.email.trim().toLocaleLowerCase())}
              removeLabel={`${label}: ${address.email}`}
              onRemove={disabled ? undefined : () => onRemove(index)}
            />
          ))}
          <input
            id={id}
            className="h-8 min-w-32 flex-1 appearance-none border-none bg-transparent px-1 text-sm text-foreground shadow-none outline-none ring-0 placeholder:text-muted-foreground/60"
            value={input}
            disabled={disabled}
            inputMode="email"
            autoComplete="off"
            spellCheck={false}
            placeholder={addresses.length ? undefined : placeholder}
            aria-invalid={Boolean(error)}
            aria-describedby={error ? errorId : undefined}
            role="combobox"
            aria-autocomplete="list"
            aria-expanded={Boolean(visibleSuggestions.length)}
            aria-controls={visibleSuggestions.length ? suggestionsId : undefined}
            aria-activedescendant={activeContact
              ? `${suggestionsId}-${activeContact.id}`
              : undefined}
            onChange={(event) => {
              setActiveSuggestion(-1);
              onInputChange(event.currentTarget.value);
            }}
            onBlur={() => { if (input.trim()) onCommit(); }}
            onKeyDown={(event) => {
              if (visibleSuggestions.length && (event.key === "ArrowDown" || event.key === "ArrowUp")) {
                event.preventDefault();
                setActiveSuggestion((current) => event.key === "ArrowDown"
                  ? (current + 1) % visibleSuggestions.length
                  : (current <= 0 || current >= visibleSuggestions.length
                    ? visibleSuggestions.length - 1
                    : current - 1));
                return;
              }
              if (event.key === "Enter" && activeSuggestion >= 0) {
                const contact = visibleSuggestions[activeSuggestion];
                if (contact) {
                  event.preventDefault();
                  selectSuggestion(contact);
                  return;
                }
              }
              const commitSeparator = event.key === "Enter" || event.key === "," || event.key === ";";
              const completeSpace = event.key === " " && input.trim().length > 0;
              if (commitSeparator || completeSpace) {
                event.preventDefault();
                onCommit();
              } else if (event.key === "Backspace" && !input && addresses.length) {
                event.preventDefault();
                const index = addresses.length - 1;
                onEditLast(addresses[index], index);
              }
            }}
          />
          {visibleSuggestions.length ? (
            <ContactSuggestions
              id={suggestionsId}
              contacts={visibleSuggestions}
              activeIndex={activeSuggestion}
              onActiveIndexChange={setActiveSuggestion}
              onSelect={selectSuggestion}
            />
          ) : null}
        </div>
        {error ? <p id={errorId} className="px-1 pt-1 text-xs text-destructive">{error}</p> : null}
      </div>
      {trailing}
    </div>
  );
}

function ContactSuggestions({ id, contacts, activeIndex, onActiveIndexChange, onSelect }: {
  id: string;
  contacts: ContactSummary[];
  activeIndex: number;
  onActiveIndexChange: (index: number) => void;
  onSelect: (contact: ContactSummary) => void;
}) {
  const { t } = useTranslation();
  return (
    <div
      id={id}
      className="absolute top-full left-0 z-50 mt-1 w-[min(28rem,calc(100vw-3rem))] overflow-hidden rounded-lg border border-border bg-popover p-1 shadow-xl"
      role="listbox"
      aria-label={t("contacts.suggestions")}
    >
      {contacts.map((contact, index) => (
        <button
          key={contact.id}
          id={`${id}-${contact.id}`}
          type="button"
          role="option"
          aria-selected={index === activeIndex}
          className={cn(
            "flex w-full cursor-pointer items-center gap-2.5 rounded-md px-3 py-2 text-left hover:bg-muted focus-visible:bg-muted focus-visible:outline-none",
            index === activeIndex && "bg-muted",
          )}
          onMouseDown={(event) => event.preventDefault()}
          onMouseEnter={() => onActiveIndexChange(index)}
          onClick={() => onSelect(contact)}
        >
          <ContactInitial name={contact.name} className="size-8 text-xs" />
          <ContactIdentity
            address={{ contactId: contact.id, name: contact.name, headerName: null, email: contact.email }}
            className="min-w-0"
            focusable={false}
          >
            <span className="block min-w-0">
              <span className="block truncate text-sm font-medium text-foreground">{contact.name}</span>
              <span className="block truncate text-xs text-muted-foreground">{contact.email}</span>
            </span>
          </ContactIdentity>
        </button>
      ))}
    </div>
  );
}

export function AddressTag({
  address,
  presentation,
  removeLabel,
  onRemove,
}: {
  address: MessageAddress;
  presentation?: AddressPresentation;
  removeLabel?: string;
  onRemove?: () => void;
}) {
  return (
    <span
      className="inline-flex min-w-0 max-w-full items-center gap-1 rounded-md bg-primary/10 py-1 pr-1 pl-2 text-xs text-primary"
      title={address.name ? `${address.name} <${address.email}>` : address.email}
    >
      <ContactIdentity address={presentation ?? address} className="min-w-0">
        <span className="inline-flex min-w-0 gap-1">
          <span className="truncate">{presentation?.name || address.name || address.email}</span>
          {presentation?.name || address.name ? <span className="truncate text-primary/70">&lt;{address.email}&gt;</span> : null}
        </span>
      </ContactIdentity>
      {onRemove ? (
        <Button
          type="button"
          variant="ghost"
          size="icon"
          className="size-5 shrink-0 text-primary hover:bg-primary/15 hover:text-primary"
          aria-label={removeLabel}
          onClick={onRemove}
        >
          <X size={12} />
        </Button>
      ) : null}
    </span>
  );
}
