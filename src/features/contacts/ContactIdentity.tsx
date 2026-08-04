import { Copy, Mail, Pencil, UserRound } from "lucide-react";
import { useTranslation } from "react-i18next";
import { useEffect, useRef, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";

import type { AddressPresentation, MessageAddress } from "@/app/types";
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuSeparator,
  ContextMenuTrigger,
} from "@/components/ui/context-menu";
import { cn } from "@/lib/utils";

type ContactAddressLike = MessageAddress | AddressPresentation;

interface ContactIdentityProps {
  address: ContactAddressLike;
  children?: ReactNode;
  className?: string;
  onOpenContact?: (contactId: string) => void;
  onEditContact?: (contactId: string) => void;
  onCopyError?: () => void;
  focusable?: boolean;
  tag?: boolean;
}

export function ContactIdentity({
  address,
  children,
  className,
  onOpenContact,
  onEditContact,
  onCopyError,
  focusable = true,
  tag = false,
}: ContactIdentityProps) {
  const { t } = useTranslation();
  const name = address.name?.trim() || null;
  const contactId = "contactId" in address ? address.contactId : null;
  const combined = name ? `${name} <${address.email}>` : address.email;
  const triggerRef = useRef<HTMLSpanElement>(null);
  const hoverTimerRef = useRef<number | null>(null);
  const [cardOpen, setCardOpen] = useState(false);
  const [cardPosition, setCardPosition] = useState({ left: 8, top: 8 });

  function positionCard() {
    const trigger = triggerRef.current;
    if (!trigger) return;
    const bounds = trigger.getBoundingClientRect();
    const width = Math.min(288, Math.max(0, window.innerWidth - 16));
    const left = Math.min(Math.max(8, bounds.left), Math.max(8, window.innerWidth - width - 8));
    const below = bounds.bottom + 8;
    const top = below + 76 <= window.innerHeight ? below : Math.max(8, bounds.top - 84);
    setCardPosition({ left, top });
  }

  function showCard() {
    if (hoverTimerRef.current !== null) {
      window.clearTimeout(hoverTimerRef.current);
      hoverTimerRef.current = null;
    }
    positionCard();
    setCardOpen(true);
  }

  function scheduleCard() {
    if (hoverTimerRef.current !== null || cardOpen) return;
    hoverTimerRef.current = window.setTimeout(() => {
      hoverTimerRef.current = null;
      showCard();
    }, 450);
  }

  function hideCard() {
    if (hoverTimerRef.current !== null) {
      window.clearTimeout(hoverTimerRef.current);
      hoverTimerRef.current = null;
    }
    setCardOpen(false);
  }

  useEffect(() => () => {
    if (hoverTimerRef.current !== null) window.clearTimeout(hoverTimerRef.current);
  }, []);

  useEffect(() => {
    if (!cardOpen) return;
    const reposition = () => positionCard();
    window.addEventListener("resize", reposition);
    window.addEventListener("scroll", reposition, true);
    return () => {
      window.removeEventListener("resize", reposition);
      window.removeEventListener("scroll", reposition, true);
    };
  }, [cardOpen]);

  function copy(value: string) {
    void writeClipboardText(value).catch(() => onCopyError?.());
  }

  return (
    <span className={cn("relative inline-flex min-w-0", className)}>
      <ContextMenu>
        <ContextMenuTrigger asChild>
          <span
            ref={triggerRef}
            className={cn(
              "min-w-0 cursor-pointer rounded-sm outline-none focus-visible:ring-2 focus-visible:ring-ring/60",
              tag && "inline-flex items-center rounded-md border border-border/70 bg-muted/55 px-2 py-1 text-foreground transition-colors hover:bg-muted",
            )}
            tabIndex={focusable ? 0 : undefined}
            aria-label={combined}
            onMouseEnter={scheduleCard}
            onMouseLeave={hideCard}
            onFocus={showCard}
            onBlur={hideCard}
            onContextMenu={hideCard}
          >
            {children ?? <span className="truncate">{name || address.email}</span>}
          </span>
        </ContextMenuTrigger>
        <ContextMenuContent>
          <ContextMenuItem disabled={!name} onSelect={() => name && copy(name)}>
            <UserRound size={15} />
            {t("contacts.copyName")}
          </ContextMenuItem>
          <ContextMenuItem onSelect={() => copy(address.email)}>
            <Mail size={15} />
            {t("contacts.copyEmail")}
          </ContextMenuItem>
          <ContextMenuItem onSelect={() => copy(combined)}>
            <Copy size={15} />
            {t("contacts.copyFullAddress")}
          </ContextMenuItem>
          {contactId && (onOpenContact || onEditContact) ? (
            <>
              <ContextMenuSeparator />
              {onOpenContact ? (
                <ContextMenuItem onSelect={() => onOpenContact(contactId)}>
                  <UserRound size={15} />
                  {t("contacts.openContact")}
                </ContextMenuItem>
              ) : null}
              {onEditContact ? (
                <ContextMenuItem onSelect={() => window.setTimeout(() => onEditContact(contactId), 0)}>
                  <Pencil size={15} />
                  {t("contacts.edit")}
                </ContextMenuItem>
              ) : null}
            </>
          ) : null}
        </ContextMenuContent>
      </ContextMenu>
      {cardOpen && typeof document !== "undefined" ? createPortal(
        <span
          role="tooltip"
          className="pointer-events-none fixed z-[70] flex w-max max-w-72 items-center gap-2.5 rounded-lg border border-border bg-popover p-3 text-popover-foreground shadow-[0_16px_40px_rgb(15_23_42/0.18)]"
          style={cardPosition}
        >
          <ContactInitial name={name || address.email} />
          <span className="min-w-0">
            <span className="block truncate text-sm font-semibold text-foreground">{name || address.email}</span>
            <span className="block truncate text-xs text-muted-foreground">{address.email}</span>
          </span>
        </span>
      , document.body) : null}
    </span>
  );
}

export function ContactInitial({ name, className }: { name: string; className?: string }) {
  const initial = name.trim().charAt(0).toLocaleUpperCase() || "?";
  return (
    <span
      aria-hidden="true"
      className={cn("grid size-9 shrink-0 place-items-center rounded-full bg-primary/12 text-sm font-bold text-primary", className)}
    >
      {initial}
    </span>
  );
}

async function writeClipboardText(value: string) {
  if (navigator.clipboard?.writeText) {
    await navigator.clipboard.writeText(value);
    return;
  }
  const textarea = document.createElement("textarea");
  textarea.value = value;
  textarea.setAttribute("readonly", "");
  textarea.style.position = "fixed";
  textarea.style.opacity = "0";
  document.body.appendChild(textarea);
  textarea.select();
  const copied = document.execCommand("copy");
  textarea.remove();
  if (!copied) throw new Error("clipboard unavailable");
}
