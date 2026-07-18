import { useMemo, useState } from "react";
import { KeyRound, LockKeyhole, Search, Server } from "lucide-react";
import { useTranslation } from "react-i18next";

import { api, normalizeCommandError } from "@/app/api";
import type {
  AccountConnectionDraft,
  AccountDraft,
  ConnectionSecurity,
  DiscoveredAccountConfig,
  ServerConfig,
} from "@/app/types";
import { Alert } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Surface } from "@/components/ui/card";
import { Checkbox } from "@/components/ui/checkbox";
import { PasswordField, TextField } from "@/components/ui/input";
import { Form, Inline, Stack } from "@/components/ui/layout";
import { SelectField } from "@/components/ui/select";
import { Divider } from "@/components/ui/separator";
import { LabelText, Text } from "@/components/ui/typography";

const blankServer = (port: number): ServerConfig => ({
  host: "",
  port,
  security: "tls",
  username: "",
});

export function PasswordAccountForm({
  initial,
  passwordRequired = true,
  submitLabel,
  onSubmit,
}: {
  initial?: AccountConnectionDraft;
  passwordRequired?: boolean;
  submitLabel: string;
  onSubmit: (draft: AccountDraft) => Promise<void>;
}) {
  const { t } = useTranslation();
  const [draft, setDraft] = useState<AccountDraft>({
    email: initial?.email ?? "",
    displayName: initial?.displayName ?? "",
    password: "",
    incoming: initial?.incoming ?? blankServer(993),
    outgoing: initial?.outgoing ?? blankServer(465),
    insecureAcknowledged: initial?.insecureAcknowledged ?? false,
  });
  const [manualVisible, setManualVisible] = useState(Boolean(initial));
  const [discovered, setDiscovered] = useState<DiscoveredAccountConfig | null>(null);
  const [discovering, setDiscovering] = useState(false);
  const [saving, setSaving] = useState(false);
  const [errorCode, setErrorCode] = useState<string | null>(null);
  const usesPlaintext = useMemo(
    () => draft.incoming.security === "none" || draft.outgoing.security === "none",
    [draft.incoming.security, draft.outgoing.security],
  );

  async function discover() {
    setDiscovering(true);
    setErrorCode(null);
    setDiscovered(null);
    try {
      const result = await api.discoverAccountConfig(draft.email);
      setDiscovered(result);
      setDraft((current) => ({ ...current, incoming: result.incoming, outgoing: result.outgoing }));
      setManualVisible(true);
    } catch (error) {
      setErrorCode(normalizeCommandError(error).code);
      setManualVisible(true);
      setDraft((current) => ({
        ...current,
        incoming: { ...current.incoming, username: current.email },
        outgoing: { ...current.outgoing, username: current.email },
      }));
    } finally {
      setDiscovering(false);
    }
  }

  async function submit() {
    setSaving(true);
    setErrorCode(null);
    try {
      await onSubmit(draft);
    } catch (error) {
      setErrorCode(normalizeCommandError(error).code);
    } finally {
      setSaving(false);
    }
  }

  function updateServer(kind: "incoming" | "outgoing", server: ServerConfig) {
    setDraft((current) => ({ ...current, [kind]: server }));
  }

  return (
    <Form onSubmit={(event) => { event.preventDefault(); void submit(); }}>
      <Stack gap="lg">
        <Surface className="rounded-md bg-muted/60 p-5 shadow-none">
          <Stack gap="lg">
            <Inline className="text-primary"><KeyRound size={18} /><LabelText>{t("onboarding.identitySection")}</LabelText></Inline>
            <TextField
              required
              type="email"
              autoComplete="email"
              label={t("onboarding.email")}
              placeholder={t("onboarding.emailPlaceholder")}
              value={draft.email}
              onChange={(event) => {
                const email = event.currentTarget.value;
                setDraft((current) => ({ ...current, email }));
              }}
            />
            <TextField
              label={t("onboarding.displayName")}
              placeholder={t("onboarding.displayNamePlaceholder")}
              value={draft.displayName}
              onChange={(event) => {
                const displayName = event.currentTarget.value;
                setDraft((current) => ({ ...current, displayName }));
              }}
            />
            <PasswordField
              required={passwordRequired}
              label={passwordRequired ? t("onboarding.password") : t("accounts.newPassword")}
              hint={passwordRequired ? t("onboarding.passwordHint") : t("accounts.newPasswordHint")}
              showPasswordLabel={t("onboarding.showPassword")}
              hidePasswordLabel={t("onboarding.hidePassword")}
              value={draft.password}
              onChange={(event) => {
                const password = event.currentTarget.value;
                setDraft((current) => ({ ...current, password }));
              }}
            />
            {!initial ? (
              <Inline className="flex-wrap justify-between">
                <Button type="button" variant="secondary" loading={discovering} disabled={!draft.email.trim()} onClick={() => void discover()}>
                  <Search size={17} />{discovering ? t("onboarding.discovering") : t("onboarding.discover")}
                </Button>
                <Button type="button" variant="ghost" onClick={() => setManualVisible((visible) => !visible)}>
                  <Server size={17} />{t("onboarding.manualSetup")}
                </Button>
              </Inline>
            ) : null}
          </Stack>
        </Surface>

        {discovered ? <Alert tone="success" title={t("onboarding.discovered")}>{sourceLabel(discovered.source, t)}</Alert> : null}

        {manualVisible ? (
          <Surface className="rounded-md bg-muted/60 p-5 shadow-none">
            <Stack gap="lg">
              <Text>{t("onboarding.manualDescription")}</Text>
              <Divider />
              <ServerFields title={t("onboarding.incoming")} server={draft.incoming} onChange={(server) => updateServer("incoming", server)} />
              <Divider />
              <ServerFields title={t("onboarding.outgoing")} server={draft.outgoing} onChange={(server) => updateServer("outgoing", server)} />
            </Stack>
          </Surface>
        ) : null}

        {usesPlaintext ? (
          <Alert tone="warning" title={t("onboarding.insecureTitle")}>
            <Stack gap="sm">
              <Text>{t("onboarding.insecureDescription")}</Text>
              <Checkbox
                checked={draft.insecureAcknowledged}
                onCheckedChange={(checked) => setDraft((current) => ({ ...current, insecureAcknowledged: checked }))}
                label={t("onboarding.insecureAcknowledge")}
              />
            </Stack>
          </Alert>
        ) : null}

        {errorCode ? <Alert title={t("errors.title")} tone="danger">{t(`errors.${errorCode}`, { defaultValue: t("common.unexpectedError") })}</Alert> : null}
        <Inline className="justify-end">
          <Button type="submit" loading={saving} disabled={!manualVisible || !draft.email || (passwordRequired && !draft.password)}>
            <LockKeyhole size={17} />{saving ? t("onboarding.verifying") : submitLabel}
          </Button>
        </Inline>
      </Stack>
    </Form>
  );
}

function ServerFields({ title, server, onChange }: { title: string; server: ServerConfig; onChange: (server: ServerConfig) => void }) {
  const { t } = useTranslation();
  return (
    <Stack gap="md">
      <Inline className="text-primary"><Server size={17} /><LabelText>{title}</LabelText></Inline>
      <Inline className="grid grid-cols-[minmax(0,1fr)_7.5rem] items-end gap-3.5 max-sm:grid-cols-1">
        <TextField required label={t("onboarding.host")} value={server.host} onChange={(event) => onChange({ ...server, host: event.currentTarget.value })} spellCheck={false} />
        <TextField required label={t("onboarding.port")} inputMode="numeric" value={String(server.port || "")} onChange={(event) => onChange({ ...server, port: Number.parseInt(event.currentTarget.value, 10) || 0 })} />
      </Inline>
      <Inline className="grid grid-cols-[minmax(0,1fr)_minmax(11.25rem,.55fr)] items-end gap-3.5 max-sm:grid-cols-1">
        <TextField required label={t("onboarding.username")} value={server.username} onChange={(event) => onChange({ ...server, username: event.currentTarget.value })} spellCheck={false} />
        <SelectField
          label={t("onboarding.security")}
          value={server.security}
          options={[
            { value: "tls", label: t("onboarding.securityTls") },
            { value: "start_tls", label: t("onboarding.securityStartTls") },
            { value: "none", label: t("onboarding.securityNone") },
          ]}
          onValueChange={(security) => onChange({ ...server, security: security as ConnectionSecurity })}
        />
      </Inline>
    </Stack>
  );
}

function sourceLabel(source: DiscoveredAccountConfig["source"], t: (key: string) => string) {
  if (source === "built_in") return t("onboarding.discoveryBuiltIn");
  if (source === "dns_srv") return t("onboarding.discoveryDns");
  return t("onboarding.discoveryAutoconfig");
}
