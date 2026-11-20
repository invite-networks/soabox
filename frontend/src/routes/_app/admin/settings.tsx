import { createFileRoute } from "@tanstack/react-router";
import { useEffect, useState } from "react";
import { errorMessage } from "../../../api/client";
import { useSettings, useUpdateSettings } from "../../../api/hooks/settings";
import type { Settings } from "../../../api/types";
import { RequirePermission } from "../../../components/forbidden";
import {
  Alert,
  Button,
  Card,
  CheckboxGroupField,
  ErrorState,
  FieldGrid,
  Form,
  LoadingState,
  PageHeader,
  SelectField,
  Stack,
  Switch,
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

function SettingsPage() {
  const settings = useSettings();
  return (
    <>
      <PageHeader title="Settings" />
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
