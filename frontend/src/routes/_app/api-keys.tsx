import { zodResolver } from "@hookform/resolvers/zod";
import { createFileRoute } from "@tanstack/react-router";
import { Pencil, Plus, Trash2 } from "lucide-react";
import { useEffect, useState } from "react";
import { Controller, useForm } from "react-hook-form";
import { z } from "zod";
import { errorMessage } from "../../api/client";
import { useAccounts } from "../../api/hooks/accounts";
import { useApiKeyAction, useApiKeys, useCreateApiKey, useUpdateApiKey } from "../../api/hooks/api-keys";
import type { ApiKey, ApiKeyInput, ApiKeyRole } from "../../api/types";
import { ConfirmDialog } from "../../components/confirm-dialog";
import { RequirePermission } from "../../components/forbidden";
import {
  Alert,
  Badge,
  Button,
  Card,
  CheckboxGroupField,
  CodeBlock,
  CopyButton,
  Dialog,
  DialogFooter,
  EmptyState,
  ErrorState,
  FieldGrid,
  Form,
  LoadingState,
  Mono,
  PageHeader,
  RowActions,
  RowHeader,
  SelectField,
  Stack,
  Table,
  TBody,
  Td,
  TextAreaField,
  TextField,
  THead,
  Th,
  Tr,
} from "../../components/ui";
import { ZonePickerField } from "../../components/zone-picker";
import { dateToUnix, formatTime, joinList, splitList, unixToDate } from "../../lib/format";
import { useMe } from "../../lib/me-context";

export const Route = createFileRoute("/_app/api-keys")({
  component: () => (
    <RequirePermission permission="create_api_keys" title="API keys">
      <ApiKeysPage />
    </RequirePermission>
  ),
});

const NONE = "none";
const CIDR = /^[0-9A-Fa-f:.]+(\/\d{1,3})?$/;

const ROLE_LABELS: Record<ApiKeyRole, string> = { admin: "Admin", operator: "Operator", scoped: "Scoped" };

const schema = z.object({
  description: z.string().trim().min(1, "Describe what the key is for."),
  role: z.enum(["admin", "operator", "scoped"]),
  zones: z.array(z.string()),
  accountIds: z.array(z.string()),
  createAccount: z.string(),
  networks: z
    .string()
    .refine(
      (v) => splitList(v).every((n) => CIDR.test(n)),
      "Enter one address or network per line, such as 192.0.2.0/24.",
    ),
  expires: z.string().refine((v) => v === "" || dateToUnix(v) !== undefined, "Enter a valid date."),
});
type Values = z.infer<typeof schema>;

function toValues(key: ApiKey | null): Values {
  return {
    description: key?.description ?? "",
    role: key?.role ?? "scoped",
    zones: key?.zones ?? [],
    accountIds: (key?.account_ids ?? []).map(String),
    createAccount: key?.create_account_id ? String(key.create_account_id) : NONE,
    networks: joinList(key?.networks ?? []),
    expires: unixToDate(key?.expires_at),
  };
}

/** Builds the request body from the form. */
export function toApiKeyInput(values: Values): ApiKeyInput {
  const scoped = values.role === "scoped";
  return {
    description: values.description.trim(),
    role: values.role,
    zones: scoped ? values.zones : [],
    account_ids: scoped ? values.accountIds.map(Number) : [],
    create_account_id: scoped && values.createAccount !== NONE ? Number(values.createAccount) : null,
    networks: splitList(values.networks),
    expires_at: dateToUnix(values.expires, true) ?? null,
  };
}

function ApiKeyDialog({
  editing,
  isOpen,
  onOpenChange,
  onCreated,
}: {
  editing: ApiKey | null;
  isOpen: boolean;
  onOpenChange: (o: boolean) => void;
  onCreated: (secret: string) => void;
}) {
  const me = useMe();
  const isAdmin = me.user.role === "admin";
  const accounts = useAccounts(isOpen);
  const create = useCreateApiKey();
  const update = useUpdateApiKey();
  const mutation = editing ? update : create;
  const form = useForm<Values>({ resolver: zodResolver(schema), defaultValues: toValues(editing) });
  const role = form.watch("role");
  const accountOptions = (accounts.data ?? []).map((a) => ({ id: String(a.id), label: a.name }));

  useEffect(() => {
    if (isOpen) {
      form.reset(toValues(editing));
      create.reset();
      update.reset();
    }
  }, [isOpen, editing, form, create.reset, update.reset]);

  const submit = form.handleSubmit((values) => {
    const body = toApiKeyInput(isAdmin ? values : { ...values, role: "scoped" });
    if (editing) {
      update.mutate({ id: editing.id, body }, { onSuccess: () => onOpenChange(false) });
    } else {
      create.mutate(body, {
        onSuccess: (result) => {
          onOpenChange(false);
          onCreated(result.secret);
        },
      });
    }
  });

  return (
    <Dialog title={editing ? "Edit API key" : "Create API key"} isOpen={isOpen} onOpenChange={onOpenChange} size="lg">
      <Form label="API key" onSubmit={submit}>
        {mutation.isError && <Alert tone="danger">{errorMessage(mutation.error)}</Alert>}
        <FieldGrid>
          <Controller
            control={form.control}
            name="description"
            render={({ field, fieldState }) => (
              <TextField
                label="Description"
                value={field.value}
                onChange={field.onChange}
                onBlur={field.onBlur}
                inputRef={field.ref}
                errorMessage={fieldState.error?.message}
              />
            )}
          />
          {isAdmin && (
            <Controller
              control={form.control}
              name="role"
              render={({ field }) => (
                <SelectField
                  label="Role"
                  value={field.value}
                  onChange={field.onChange}
                  options={[
                    { id: "scoped", label: "Scoped" },
                    { id: "operator", label: "Operator" },
                    { id: "admin", label: "Admin" },
                  ]}
                />
              )}
            />
          )}
        </FieldGrid>
        {role === "scoped" && (
          <>
            <Controller
              control={form.control}
              name="zones"
              render={({ field }) => (
                <ZonePickerField
                  description="The key can act on these zones."
                  value={field.value}
                  onChange={field.onChange}
                />
              )}
            />
            <Controller
              control={form.control}
              name="accountIds"
              render={({ field }) => (
                <CheckboxGroupField
                  label="Accounts"
                  description="The key can also act on every zone of these accounts."
                  value={field.value}
                  onChange={field.onChange}
                  options={accountOptions}
                  columns={3}
                />
              )}
            />
            <Controller
              control={form.control}
              name="createAccount"
              render={({ field }) => (
                <SelectField
                  label="Zone creation"
                  description="Zones the key creates are assigned to this account."
                  value={field.value}
                  onChange={field.onChange}
                  options={[{ id: NONE, label: "Not allowed" }, ...accountOptions]}
                />
              )}
            />
          </>
        )}
        <FieldGrid>
          <Controller
            control={form.control}
            name="networks"
            render={({ field, fieldState }) => (
              <TextAreaField
                label="Allowed networks"
                description="One address or network per line. Leave empty to allow any."
                value={field.value}
                onChange={field.onChange}
                inputRef={field.ref}
                errorMessage={fieldState.error?.message}
                mono
              />
            )}
          />
          <Controller
            control={form.control}
            name="expires"
            render={({ field, fieldState }) => (
              <TextField
                label="Expires"
                type="date"
                description="Leave empty for no expiry."
                value={field.value}
                onChange={field.onChange}
                inputRef={field.ref}
                errorMessage={fieldState.error?.message}
              />
            )}
          />
        </FieldGrid>
        <DialogFooter>
          <Button onPress={() => onOpenChange(false)}>Cancel</Button>
          <Button type="submit" variant="primary" isPending={mutation.isPending}>
            {editing ? "Save key" : "Create key"}
          </Button>
        </DialogFooter>
      </Form>
    </Dialog>
  );
}

function SecretDialog({ secret, onClose }: { secret: string | null; onClose: () => void }) {
  return (
    <Dialog
      title="Copy your new API key"
      isOpen={secret !== null}
      onOpenChange={(o) => !o && onClose()}
      isDismissable={false}
    >
      <div className="flex flex-col gap-3">
        <Alert tone="warning" title="This key is shown only once">
          Copy it now and store it somewhere safe. If you lose it, create a new key.
        </Alert>
        <CodeBlock label="API key">{secret}</CodeBlock>
        <div>
          <CopyButton text={secret ?? ""} label="Copy key" />
        </div>
      </div>
      <DialogFooter>
        <Button variant="primary" onPress={onClose}>
          I have copied the key
        </Button>
      </DialogFooter>
    </Dialog>
  );
}

function KeyStatus({ apiKey }: { apiKey: ApiKey }) {
  if (apiKey.disabled) {
    return <Badge>Disabled</Badge>;
  }
  if (apiKey.expires_at !== null && apiKey.expires_at * 1000 < Date.now()) {
    return <Badge tone="warning">Expired</Badge>;
  }
  return <Badge tone="success">Active</Badge>;
}

function ApiKeysPage() {
  const keys = useApiKeys();
  const action = useApiKeyAction();
  const [dialog, setDialog] = useState<{ open: boolean; editing: ApiKey | null }>({ open: false, editing: null });
  const [secret, setSecret] = useState<string | null>(null);
  const [deleting, setDeleting] = useState<ApiKey | null>(null);
  const apiExample = `curl -H "X-API-Key: $SOABOX_KEY" \\\n  ${window.location.origin}/api/v1/servers/localhost/zones`;

  return (
    <>
      <PageHeader
        title="API keys"
        description="Keys for automation that uses the PowerDNS-compatible API."
        actions={
          <Button variant="primary" onPress={() => setDialog({ open: true, editing: null })}>
            <Plus aria-hidden />
            Create API key
          </Button>
        }
      />
      {action.isError && (
        <div className="mb-3">
          <Alert tone="danger">{errorMessage(action.error)}</Alert>
        </div>
      )}
      <Stack gap="lg">
        <Card flush>
          {keys.isPending ? (
            <LoadingState />
          ) : keys.isError ? (
            <ErrorState error={keys.error} onRetry={() => void keys.refetch()} />
          ) : keys.data.length === 0 ? (
            <EmptyState title="No API keys yet" />
          ) : (
            <Table label="API keys">
              <THead>
                <tr>
                  <Th>Key</Th>
                  <Th>Description</Th>
                  <Th>Role</Th>
                  <Th>Scope</Th>
                  <Th>Expires</Th>
                  <Th>Last used</Th>
                  <Th>Status</Th>
                  <Th hidden>Actions</Th>
                </tr>
              </THead>
              <TBody>
                {keys.data.map((k) => (
                  <Tr key={k.id}>
                    <RowHeader>
                      <Mono>...{k.hint}</Mono>
                    </RowHeader>
                    <Td wrap>{k.description}</Td>
                    <Td>{ROLE_LABELS[k.role] ?? k.role}</Td>
                    <Td>
                      {k.role === "scoped"
                        ? `${k.zones.length} ${k.zones.length === 1 ? "zone" : "zones"}, ${k.account_ids.length} ${k.account_ids.length === 1 ? "account" : "accounts"}`
                        : "All zones"}
                    </Td>
                    <Td>{k.expires_at ? formatTime(k.expires_at) : "Never"}</Td>
                    <Td>{formatTime(k.last_used_at)}</Td>
                    <Td>
                      <KeyStatus apiKey={k} />
                    </Td>
                    <Td align="right">
                      <RowActions>
                        <Button
                          size="sm"
                          onPress={() => action.mutate({ id: k.id, action: k.disabled ? "enable" : "disable" })}
                        >
                          {k.disabled ? "Enable" : "Disable"}
                        </Button>
                        <Button
                          variant="ghost"
                          size="icon"
                          aria-label={`Edit key ...${k.hint}`}
                          onPress={() => setDialog({ open: true, editing: k })}
                        >
                          <Pencil aria-hidden />
                        </Button>
                        <Button
                          variant="ghost"
                          size="icon"
                          aria-label={`Delete key ...${k.hint}`}
                          onPress={() => setDeleting(k)}
                        >
                          <Trash2 aria-hidden />
                        </Button>
                      </RowActions>
                    </Td>
                  </Tr>
                ))}
              </TBody>
            </Table>
          )}
        </Card>
        <Card title="Using the PowerDNS API">
          <div className="flex flex-col gap-3 text-sm text-fg-muted">
            <p>
              Existing PowerDNS tools work unchanged. Point them at SOAbox and send a SOAbox API key in the{" "}
              <Mono>X-API-Key</Mono> header. Every change is checked against the key&apos;s scope and recorded in
              history.
            </p>
            <CodeBlock label="Example request">{apiExample}</CodeBlock>
          </div>
        </Card>
      </Stack>
      <ApiKeyDialog
        editing={dialog.editing}
        isOpen={dialog.open}
        onOpenChange={(open) => setDialog((d) => ({ ...d, open }))}
        onCreated={setSecret}
      />
      <SecretDialog secret={secret} onClose={() => setSecret(null)} />
      <ConfirmDialog
        title="Delete API key"
        isOpen={deleting !== null}
        onOpenChange={(o) => !o && setDeleting(null)}
        confirmLabel="Delete key"
        isPending={action.isPending}
        error={action.error}
        onConfirm={() =>
          deleting && action.mutate({ id: deleting.id, action: "delete" }, { onSuccess: () => setDeleting(null) })
        }
      >
        <p>Delete the key ending in {deleting?.hint}? Automation using it stops working immediately.</p>
      </ConfirmDialog>
    </>
  );
}
