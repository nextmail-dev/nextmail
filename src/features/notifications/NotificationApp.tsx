import { listen } from "@tauri-apps/api/event";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Mail, X } from "lucide-react";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { api, normalizeCommandError } from "@/app/api";
import { useAppearancePreferences } from "@/app/appearance";
import type { NewMailNotification } from "@/app/types";
import { Alert } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { IconTile } from "@/components/ui/icon-tile";
import { AppShell, Stack } from "@/components/ui/layout";
import { Spinner } from "@/components/ui/spinner";
import { LabelText, Text } from "@/components/ui/typography";

export const notificationQueryKey = (notificationId: string) => (
  ["new-mail-notification", notificationId] as const
);

export function NotificationApp({ notificationId }: { notificationId: string }) {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const [bridgeReady, setBridgeReady] = useState(false);
  const [actionError, setActionError] = useState<string | null>(null);
  const [pending, setPending] = useState(false);
  useAppearancePreferences();

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | null = null;
    void listen<NewMailNotification>("notification-content-changed", (event) => {
      if (event.payload.id !== notificationId) return;
      queryClient.setQueryData(notificationQueryKey(notificationId), event.payload);
      setActionError(null);
      setPending(false);
    }).then((dispose) => {
      if (disposed) dispose();
      else {
        unlisten = dispose;
        setBridgeReady(true);
      }
    }).catch(() => {
      if (!disposed) setBridgeReady(true);
    });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [notificationId, queryClient]);

  const notificationQuery = useQuery({
    queryKey: notificationQueryKey(notificationId),
    queryFn: () => api.getNewMailNotification(notificationId),
    enabled: bridgeReady,
    retry: false,
    staleTime: Number.POSITIVE_INFINITY,
  });

  function activate() {
    if (pending) return;
    setPending(true);
    setActionError(null);
    void api.activateNewMailNotification(notificationId).catch((error) => {
      setPending(false);
      setActionError(normalizeCommandError(error).code);
    });
  }

  function dismiss() {
    if (pending) return;
    setPending(true);
    setActionError(null);
    void api.dismissNewMailNotification(notificationId).catch((error) => {
      setPending(false);
      setActionError(normalizeCommandError(error).code);
    });
  }

  if (!bridgeReady || notificationQuery.isPending) {
    return <AppShell className="grid place-items-center bg-card"><Spinner size={20} /></AppShell>;
  }
  const notification = notificationQuery.data;
  if (!notification) {
    const error = normalizeCommandError(notificationQuery.error);
    return (
      <AppShell className="grid place-items-center bg-card p-3">
        <Alert tone="danger">{t(`errors.${error.code}`, { defaultValue: t("common.unexpectedError") })}</Alert>
      </AppShell>
    );
  }

  const account = notification.accountName || notification.accountEmail;
  return (
    <AppShell className="relative overflow-hidden bg-card">
      <Button
        variant="ghost"
        className="h-full w-full justify-start rounded-none px-4 py-3 pr-11 text-left whitespace-normal hover:bg-primary/6"
        aria-label={t("notifications.openMessage", { subject: notification.subject || t("mail.noSubject") })}
        disabled={pending}
        onClick={activate}
      >
        <IconTile><Mail size={19} /></IconTile>
        <Stack className="min-w-0 flex-1" gap="xs">
          <Text className="truncate text-xs font-semibold text-primary">
            {t("notifications.newMailForAccount", { account })}
          </Text>
          <LabelText className="truncate">{formatNotificationSender(notification)}</LabelText>
          <Text className="line-clamp-2 text-sm leading-snug text-foreground">
            {notification.subject || t("mail.noSubject")}
          </Text>
        </Stack>
      </Button>
      <Button
        variant="ghost"
        size="icon"
        className="absolute top-2 right-2 z-10 size-7"
        aria-label={t("notifications.dismiss")}
        disabled={pending}
        onClick={dismiss}
      >
        <X size={15} />
      </Button>
      {actionError ? (
        <Alert className="absolute right-2 bottom-2 left-2 z-20 py-2" tone="danger">
          {t(`errors.${actionError}`, { defaultValue: t("common.unexpectedError") })}
        </Alert>
      ) : null}
    </AppShell>
  );
}

export function formatNotificationSender(notification: Pick<NewMailNotification, "senderName" | "senderEmail">) {
  const name = notification.senderName?.trim();
  if (!name) return notification.senderEmail;
  if (!notification.senderEmail) return name;
  return `${name} <${notification.senderEmail}>`;
}
