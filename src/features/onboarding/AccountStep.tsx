import { useMemo, useState } from "react";
import { KeyRound, LockKeyhole, Search, Server, ShieldCheck } from "lucide-react";
import { useTranslation } from "react-i18next";
import { api, normalizeCommandError } from "@/app/api";
import type {
  AccountDraft,
  AppearancePreferences,
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
import { Eyebrow, Heading, LabelText, Text } from "@/components/ui/typography";
import { OnboardingLayout } from "./OnboardingLayout";

const blankServer = (port: number): ServerConfig => ({
  host: "",
  port,
  security: "tls",
  username: "",
});

interface AccountStepProps {
  preferences: AppearancePreferences;
  onPreferencesChange: (preferences: AppearancePreferences) => void;
  onCompleted: () => void;
}

export function AccountStep({ preferences, onPreferencesChange, onCompleted }: AccountStepProps) {
  const { t } = useTranslation();
  const [draft, setDraft] = useState<AccountDraft>({
    email: "",
    displayName: "",
    password: "",
    incoming: blankServer(993),
    outgoing: blankServer(465),
    insecureAcknowledged: false,
  });
  const [manualVisible, setManualVisible] = useState(false);
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
      await api.savePasswordAccount(draft);
      await api.completeOnboarding();
      onCompleted();
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
    <OnboardingLayout
      activeStep={2}
      preferences={preferences}
      onPreferencesChange={onPreferencesChange}
      aside={
        <Stack className="mt-2 rounded-sm bg-primary/10 p-4 text-primary" gap="sm">
          <ShieldCheck size={28} aria-hidden="true" />
          <Text className="text-xs">{t("onboarding.privacyNote")}</Text>
        </Stack>
      }
    >
      <Form
        className="mx-auto max-w-3xl pb-12"
        onSubmit={(event) => {
          event.preventDefault();
          void submit();
        }}
      >
        <Stack gap="xl">
          <Stack gap="sm">
            <Eyebrow>{t("onboarding.accountEyebrow")}</Eyebrow>
            <Heading>{t("onboarding.accountTitle")}</Heading>
            <Text className="max-w-2xl text-base leading-relaxed">
              {t("onboarding.accountDescription")}
            </Text>
          </Stack>

          <Surface className="rounded-md p-5">
            <Stack gap="lg">
              <Inline className="text-primary">
                <KeyRound size={18} />
                <LabelText>{t("onboarding.identitySection")}</LabelText>
              </Inline>
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
                required
                label={t("onboarding.password")}
                hint={t("onboarding.passwordHint")}
                showPasswordLabel={t("onboarding.showPassword")}
                hidePasswordLabel={t("onboarding.hidePassword")}
                value={draft.password}
                onChange={(event) => {
                  const password = event.currentTarget.value;
                  setDraft((current) => ({ ...current, password }));
                }}
              />
              <Inline className="flex-wrap justify-between">
                <Button
                  type="button"
                  variant="secondary"
                  loading={discovering}
                  disabled={!draft.email.trim()}
                  onClick={() => void discover()}
                >
                  <Search size={17} />
                  {discovering ? t("onboarding.discovering") : t("onboarding.discover")}
                </Button>
                <Button
                  type="button"
                  variant="ghost"
                  onClick={() => setManualVisible((visible) => !visible)}
                >
                  <Server size={17} />
                  {t("onboarding.manualSetup")}
                </Button>
              </Inline>
            </Stack>
          </Surface>

          {discovered ? (
            <Alert tone="success" title={t("onboarding.discovered")}>
              {sourceLabel(discovered.source, t)}
            </Alert>
          ) : null}

          {manualVisible ? (
            <Surface className="rounded-md bg-muted/60 p-5">
              <Stack gap="lg">
                <Text>{t("onboarding.manualDescription")}</Text>
                <Divider />
                <ServerFields
                  title={t("onboarding.incoming")}
                  server={draft.incoming}
                  onChange={(server) => updateServer("incoming", server)}
                />
                <Divider />
                <ServerFields
                  title={t("onboarding.outgoing")}
                  server={draft.outgoing}
                  onChange={(server) => updateServer("outgoing", server)}
                />
              </Stack>
            </Surface>
          ) : null}

          {usesPlaintext ? (
            <Alert tone="warning" title={t("onboarding.insecureTitle")}>
              <Stack gap="sm">
                <Text>{t("onboarding.insecureDescription")}</Text>
                <Checkbox
                  checked={draft.insecureAcknowledged}
                  onCheckedChange={(checked) =>
                    setDraft((current) => ({ ...current, insecureAcknowledged: checked }))
                  }
                  label={t("onboarding.insecureAcknowledge")}
                />
              </Stack>
            </Alert>
          ) : null}

          {errorCode ? (
            <Alert title={t("errors.title")} tone="danger">
              {t(`errors.${errorCode}`, { defaultValue: t("common.unexpectedError") })}
            </Alert>
          ) : null}

          <Inline className="flex-wrap justify-end">
            <Button
              type="submit"
              size="lg"
              loading={saving}
              disabled={!manualVisible || !draft.email || !draft.password}
            >
              <LockKeyhole size={18} />
              {saving ? t("onboarding.verifying") : t("onboarding.verifyAndFinish")}
            </Button>
          </Inline>
        </Stack>
      </Form>
    </OnboardingLayout>
  );
}

function ServerFields({
  title,
  server,
  onChange,
}: {
  title: string;
  server: ServerConfig;
  onChange: (server: ServerConfig) => void;
}) {
  const { t } = useTranslation();
  const securityOptions = [
    { value: "tls", label: t("onboarding.securityTls") },
    { value: "start_tls", label: t("onboarding.securityStartTls") },
    { value: "none", label: t("onboarding.securityNone") },
  ];
  return (
    <Stack gap="md">
      <Inline className="text-primary">
        <Server size={17} />
        <LabelText>{title}</LabelText>
      </Inline>
      <Inline className="grid grid-cols-[minmax(0,1fr)_7.5rem] items-end gap-3.5 max-sm:grid-cols-1">
        <TextField
          required
          label={t("onboarding.host")}
          value={server.host}
          onChange={(event) => {
            const host = event.currentTarget.value;
            onChange({ ...server, host });
          }}
          spellCheck={false}
        />
        <TextField
          required
          label={t("onboarding.port")}
          inputMode="numeric"
          value={String(server.port || "")}
          onChange={(event) => {
            const port = Number.parseInt(event.currentTarget.value, 10) || 0;
            onChange({ ...server, port });
          }}
        />
      </Inline>
      <Inline className="grid grid-cols-[minmax(0,1fr)_minmax(11.25rem,.55fr)] items-end gap-3.5 max-sm:grid-cols-1">
        <TextField
          required
          label={t("onboarding.username")}
          value={server.username}
          onChange={(event) => {
            const username = event.currentTarget.value;
            onChange({ ...server, username });
          }}
          spellCheck={false}
        />
        <SelectField
          label={t("onboarding.security")}
          value={server.security}
          options={securityOptions}
          onValueChange={(security) =>
            onChange({ ...server, security: security as ConnectionSecurity })
          }
        />
      </Inline>
    </Stack>
  );
}

function sourceLabel(
  source: DiscoveredAccountConfig["source"],
  t: (key: string) => string,
) {
  if (source === "built_in") return t("onboarding.discoveryBuiltIn");
  if (source === "dns_srv") return t("onboarding.discoveryDns");
  return t("onboarding.discoveryAutoconfig");
}
