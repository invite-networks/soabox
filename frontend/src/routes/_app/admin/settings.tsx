import { createFileRoute } from "@tanstack/react-router";
import { useEffect, useState } from "react";
import { errorMessage } from "../../../api/client";
import { useOidcSettings, useSettings, useUpdateOidcSettings, useUpdateSettings } from "../../../api/hooks/settings";
import type { OidcSettings, OidcSettingsUpdate, Settings } from "../../../api/types";
import { RequirePermission } from "../../../components/forbidden";
import {
  Alert,
  Button,
  Card,
  CheckboxGroupField,
  CopyButton,
  DetailList,
  ErrorState,
  FieldGrid,
  Form,
  LoadingState,
  Mono,
  PageHeader,
  SelectField,
  Stack,
  Switch,
  TextField,
} from "../../../components/ui";
import { KNOWN_RECORD_TYPES } from "../../../lib/dns";

export const Route = createFileRoute("/_app/admin/settings")({
  component: () => (
    <RequirePermission permission="manage_settings" title="Settings">
      <SettingsPage />
    </RequirePermission>
  ),
});

type BooleanKey = {
  [K in keyof Settings]: Settings[K] extends boolean ? K : never;
}[keyof Settings];

const USER_PERMISSIONS: { key: BooleanKey; label: string }[] = [
  { key: "user_can_create_zones", label: "Users can create zones" },
  { key: "user_can_delete_zones", label: "Users can delete their zones" },
  { key: "user_can_view_history", label: "Users can view history of their zones" },
  { key: "user_can_manage_dnssec", label: "Users can manage DNSSEC on their zones" },
  { key: "user_can_create_api_keys", label: "Users can create their own API keys" },
];

function typeOptions(current: readonly string[]) {
  return [...new Set([...KNOWN_RECORD_TYPES, ...current])].sort().map((t) => ({ id: t, label: t }));
}

function SettingsForm({ initial }: { initial: Settings }) {
  const update = useUpdateSettings();
  const [values, setValues] = useState<Settings>(initial);
  const [saved, setSaved] = useState(false);
  useEffect(() => setValues(initial), [initial]);
  const set = <K extends keyof Settings>(key: K, value: Settings[K]) => {
    setSaved(false);
    setValues((v) => ({ ...v, [key]: value }));
  };

  return (
    <Form label="Settings" onSubmit={() => update.mutate(values, { onSuccess: () => setSaved(true) })}>
      <Stack>
        {update.isError && <Alert tone="danger">{errorMessage(update.error)}</Alert>}
        {saved && <Alert tone="success">Settings saved.</Alert>}
        <Card title="Sign-in">
          <FieldGrid>
            <SelectField
              label="Local sign-in when single sign-on is on"
              value={values.local_login_with_oidc}
              onChange={(v) => set("local_login_with_oidc", v === "deny" ? "deny" : "allow")}
              options={[
                { id: "allow", label: "Allow" },
                { id: "deny", label: "Deny, except for admins" },
              ]}
            />
            <SelectField
              label="New single sign-on users"
              value={values.oidc_new_user}
              onChange={(v) => set("oidc_new_user", v === "approve" ? "approve" : "create")}
              options={[
                { id: "create", label: "Create as User" },
                { id: "approve", label: "Hold for approval" },
              ]}
            />
          </FieldGrid>
        </Card>
        <Card title="User permissions">
          <div className="flex flex-col gap-3">
            {USER_PERMISSIONS.map((p) => (
              <Switch key={p.key} isSelected={values[p.key]} onChange={(v) => set(p.key, v)}>
                {p.label}
              </Switch>
            ))}
          </div>
        </Card>
        <Card title="Records">
          <div className="flex flex-col gap-4">
            <Switch
              isSelected={values.auto_ptr}
              onChange={(v) => set("auto_ptr", v)}
              description="When an A or AAAA record changes, update the matching PTR in a reverse zone the user can edit."
            >
              Automatic PTR records
            </Switch>
            <CheckboxGroupField
              label="Record types for forward zones"
              value={values.record_types_forward}
              onChange={(v) => set("record_types_forward", v)}
              options={typeOptions(values.record_types_forward)}
              columns={4}
            />
            <CheckboxGroupField
              label="Record types for reverse zones"
              value={values.record_types_reverse}
              onChange={(v) => set("record_types_reverse", v)}
              options={typeOptions(values.record_types_reverse)}
              columns={4}
            />
          </div>
        </Card>
        <div>
          <Button type="submit" variant="primary" isPending={update.isPending}>
            Save settings
          </Button>
        </div>
      </Stack>
    </Form>
  );
}

type SsoErrors = Partial<Record<"issuer" | "client_id" | "client_secret" | "button_label", string>>;

function validateSso(v: OidcSettingsUpdate, secretSet: boolean): SsoErrors {
  const errors: SsoErrors = {};
  const issuer = v.issuer.trim();
  if (issuer) {
    let url: URL | null = null;
    try {
      url = new URL(issuer);
    } catch {
      url = null;
    }
    if (!url || (url.protocol !== "https:" && url.protocol !== "http:")) {
      errors.issuer = "Enter the issuer as a full https URL.";
    }
  } else if (v.enabled) {
    errors.issuer = "Enter the issuer URL.";
  }
  if (v.enabled && !v.client_id.trim()) {
    errors.client_id = "Enter the client ID.";
  }
  if (v.enabled && !secretSet && !v.client_secret?.trim()) {
    errors.client_secret = "Enter the client secret.";
  }
  if (v.button_label.trim().length > 64) {
    errors.button_label = "Use up to 64 characters.";
  }
  return errors;
}

function SsoForm({ initial }: { initial: OidcSettings }) {
  const update = useUpdateOidcSettings();
  const fromEnv = initial.source === "environment";
  const [values, setValues] = useState<OidcSettingsUpdate>({
    enabled: initial.enabled,
    issuer: initial.issuer,
    client_id: initial.client_id,
    button_label: initial.button_label,
  });
  const [replacingSecret, setReplacingSecret] = useState(false);
  const [secret, setSecret] = useState("");
  const [errors, setErrors] = useState<SsoErrors>({});
  const [saved, setSaved] = useState(false);
  const showSecretField = !fromEnv && (!initial.client_secret_set || replacingSecret);
  const set = <K extends keyof OidcSettingsUpdate>(key: K, value: OidcSettingsUpdate[K]) => {
    setSaved(false);
    setValues((v) => ({ ...v, [key]: value }));
  };

  const submit = () => {
    const body: OidcSettingsUpdate = { ...values, ...(secret.trim() ? { client_secret: secret.trim() } : {}) };
    const next = validateSso(body, initial.client_secret_set);
    setErrors(next);
    if (Object.keys(next).length > 0) {
      return;
    }
    update.mutate(body, {
      onSuccess: (result) => {
        setValues({
          enabled: result.enabled,
          issuer: result.issuer,
          client_id: result.client_id,
          button_label: result.button_label,
        });
        setSaved(true);
        setSecret("");
        setReplacingSecret(false);
      },
    });
  };

  return (
    <Card title="Single sign-on">
      <Form label="Single sign-on" onSubmit={submit}>
        <Stack>
          {fromEnv && (
            <Alert title="Set by environment">
              These values come from the SOABOX_OIDC_* environment variables and take precedence. Remove them and
              restart to manage single sign-on here.
            </Alert>
          )}
          {update.isError && <Alert tone="danger">{errorMessage(update.error)}</Alert>}
          {saved && <Alert tone="success">Single sign-on saved.</Alert>}
          <DetailList
            items={[
              {
                label: "Redirect URI",
                value: (
                  <span className="inline-flex flex-wrap items-center gap-2">
                    <Mono>{initial.redirect_uri}</Mono>
                    <CopyButton text={initial.redirect_uri} label="Copy redirect URI" />
                  </span>
                ),
              },
            ]}
          />
          <Switch
            isSelected={values.enabled}
            onChange={(v) => set("enabled", v)}
            isDisabled={fromEnv}
            description="Shows the single sign-on button on the sign-in page."
          >
            Single sign-on enabled
          </Switch>
          <FieldGrid>
            <TextField
              label="Issuer URL"
              type="url"
              mono
              value={values.issuer}
              onChange={(v) => set("issuer", v)}
              isReadOnly={fromEnv}
              placeholder="https://login.microsoftonline.com/<tenant-id>/v2.0"
              description="For Microsoft Entra ID, use the tenant-specific v2.0 endpoint."
              errorMessage={errors.issuer}
            />
            <TextField
              label="Client ID"
              mono
              value={values.client_id}
              onChange={(v) => set("client_id", v)}
              isReadOnly={fromEnv}
              errorMessage={errors.client_id}
            />
            {showSecretField ? (
              <TextField
                label="Client secret"
                type="password"
                autoComplete="new-password"
                value={secret}
                onChange={(v) => {
                  setSaved(false);
                  setSecret(v);
                }}
                {...(initial.client_secret_set ? { description: "Leave empty to keep the current secret." } : {})}
                errorMessage={errors.client_secret}
              />
            ) : (
              <div className="flex flex-col gap-1">
                <span className="text-sm font-medium text-fg">Client secret</span>
                <span className="inline-flex flex-wrap items-center gap-2 text-sm text-fg-muted">
                  Secret is set
                  {!fromEnv && (
                    <Button size="sm" onPress={() => setReplacingSecret(true)}>
                      Replace secret
                    </Button>
                  )}
                </span>
              </div>
            )}
            <TextField
              label="Button label"
              value={values.button_label}
              onChange={(v) => set("button_label", v)}
              isReadOnly={fromEnv}
              errorMessage={errors.button_label}
            />
          </FieldGrid>
          {!fromEnv && (
            <div>
              <Button type="submit" variant="primary" isPending={update.isPending}>
                Save single sign-on
              </Button>
            </div>
          )}
        </Stack>
      </Form>
    </Card>
  );
}

function SsoSection() {
  const oidc = useOidcSettings();
  if (oidc.isPending) {
    return <LoadingState label="Loading single sign-on" />;
  }
  if (oidc.isError) {
    return <ErrorState error={oidc.error} onRetry={() => void oidc.refetch()} />;
  }
  return <SsoForm initial={oidc.data} />;
}

function SettingsPage() {
  const settings = useSettings();
  return (
    <>
      <PageHeader title="Settings" />
      <div className="mb-4">
        <SsoSection />
      </div>
      {settings.isPending ? (
        <LoadingState />
      ) : settings.isError ? (
        <ErrorState error={settings.error} onRetry={() => void settings.refetch()} />
      ) : (
        <SettingsForm initial={settings.data} />
      )}
    </>
  );
}
