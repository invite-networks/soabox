import { zodResolver } from "@hookform/resolvers/zod";
import { createFileRoute } from "@tanstack/react-router";
import { MoreHorizontal, Plus } from "lucide-react";
import { useEffect, useState } from "react";
import { Controller, useForm } from "react-hook-form";
import { z } from "zod";
import { errorMessage } from "../../../api/client";
import { useAccounts } from "../../../api/hooks/accounts";
import {
  type UserAction,
  useCreateUser,
  useResetUserPassword,
  useUpdateUser,
  useUserAction,
  useUsers,
} from "../../../api/hooks/users";
import type { Role, User, UserStatus } from "../../../api/types";
import { ConfirmDialog } from "../../../components/confirm-dialog";
import { RequirePermission } from "../../../components/forbidden";
import {
  Alert,
  Badge,
  Button,
  Card,
  CheckboxGroupField,
  Dialog,
  DialogFooter,
  EmptyState,
  ErrorState,
  FieldGrid,
  Form,
  LoadingState,
  Menu,
  MenuItem,
  MenuTrigger,
  PageHeader,
  RowActions,
  RowHeader,
  SelectField,
  Table,
  TBody,
  Td,
  TextAreaField,
  TextField,
  THead,
  Th,
  type Tone,
  Tr,
} from "../../../components/ui";
import { isHostname } from "../../../lib/dns";
import { formatTime, joinList, splitList } from "../../../lib/format";
import { useMe } from "../../../lib/me-context";

export const Route = createFileRoute("/_app/admin/users")({
  component: () => (
    <RequirePermission permission="manage_users" title="Users">
      <UsersPage />
    </RequirePermission>
  ),
});

const ROLE_OPTIONS = [
  { id: "admin", label: "Admin" },
  { id: "operator", label: "Operator" },
  { id: "user", label: "User" },
];
const ROLE_LABELS: Record<Role, string> = { admin: "Admin", operator: "Operator", user: "User" };
const STATUS: Record<UserStatus, { label: string; tone: Tone }> = {
  active: { label: "Active", tone: "success" },
  pending: { label: "Pending approval", tone: "warning" },
  disabled: { label: "Disabled", tone: "neutral" },
};
const MIN_PASSWORD = 12;

const createSchema = z.object({
  username: z.string().trim().min(1, "Enter a username."),
  display_name: z.string().trim(),
  email: z.union([z.literal(""), z.email("Enter a valid email address.")]),
  role: z.enum(["admin", "operator", "user"]),
  password: z.string().min(MIN_PASSWORD, `Use at least ${MIN_PASSWORD} characters.`),
});
type CreateValues = z.infer<typeof createSchema>;
const EMPTY_USER: CreateValues = { username: "", display_name: "", email: "", role: "user", password: "" };

function CreateUserDialog({ isOpen, onOpenChange }: { isOpen: boolean; onOpenChange: (o: boolean) => void }) {
  const create = useCreateUser();
  const form = useForm<CreateValues>({ resolver: zodResolver(createSchema), defaultValues: EMPTY_USER });
  const resetForm = form.reset;
  const resetMutation = create.reset;
  useEffect(() => {
    if (isOpen) {
      resetForm(EMPTY_USER);
      resetMutation();
    }
  }, [isOpen, resetForm, resetMutation]);
  const submit = form.handleSubmit((values) => create.mutate(values, { onSuccess: () => onOpenChange(false) }));
  const text = (
    name: "username" | "display_name" | "email" | "password",
    label: string,
    type?: "password" | "email",
  ) => (
    <Controller
      control={form.control}
      name={name}
      render={({ field, fieldState }) => (
        <TextField
          label={label}
          {...(type ? { type } : {})}
          value={field.value}
          onChange={field.onChange}
          onBlur={field.onBlur}
          inputRef={field.ref}
          errorMessage={fieldState.error?.message}
          autoComplete={type === "password" ? "new-password" : "off"}
        />
      )}
    />
  );
  return (
    <Dialog title="Create local user" isOpen={isOpen} onOpenChange={onOpenChange}>
      <Form label="Create user" onSubmit={submit}>
        {create.isError && <Alert tone="danger">{errorMessage(create.error)}</Alert>}
        <FieldGrid>
          {text("username", "Username")}
          {text("display_name", "Display name")}
          {text("email", "Email", "email")}
          <Controller
            control={form.control}
            name="role"
            render={({ field }) => (
              <SelectField label="Role" value={field.value} onChange={field.onChange} options={ROLE_OPTIONS} />
            )}
          />
        </FieldGrid>
        {text("password", "Password", "password")}
        <DialogFooter>
          <Button onPress={() => onOpenChange(false)}>Cancel</Button>
          <Button type="submit" variant="primary" isPending={create.isPending}>
            Create user
          </Button>
        </DialogFooter>
      </Form>
    </Dialog>
  );
}

const editSchema = z.object({
  display_name: z.string().trim(),
  email: z.union([z.literal(""), z.email("Enter a valid email address.")]),
  role: z.enum(["admin", "operator", "user"]),
  status: z.enum(["active", "pending", "disabled"]),
  zones: z.string().refine((v) => splitList(v).every((zone) => isHostname(zone)), "Enter one zone name per line."),
  account_ids: z.array(z.string()),
});
type EditValues = z.infer<typeof editSchema>;

function toEditValues(user: User): EditValues {
  return {
    display_name: user.display_name,
    email: user.email,
    role: user.role,
    status: user.status,
    zones: joinList(user.zones),
    account_ids: user.account_ids.map(String),
  };
}

function EditUserDialog({ user, onClose }: { user: User | null; onClose: () => void }) {
  const update = useUpdateUser();
  const accounts = useAccounts(user !== null);
  const form = useForm<EditValues>({
    resolver: zodResolver(editSchema),
    ...(user ? { defaultValues: toEditValues(user) } : {}),
  });
  const resetForm = form.reset;
  const resetMutation = update.reset;
  useEffect(() => {
    if (user) {
      resetForm(toEditValues(user));
      resetMutation();
    }
  }, [user, resetForm, resetMutation]);
  const submit = form.handleSubmit((values) => {
    if (!user) {
      return;
    }
    update.mutate(
      {
        id: user.id,
        body: {
          display_name: values.display_name,
          email: values.email,
          role: values.role,
          status: values.status,
          zones: splitList(values.zones).map((zone) => (zone.endsWith(".") ? zone : `${zone}.`)),
          account_ids: values.account_ids.map(Number),
        },
      },
      { onSuccess: onClose },
    );
  });
  return (
    <Dialog
      title={user ? `Edit ${user.username}` : "Edit user"}
      isOpen={user !== null}
      onOpenChange={(o) => !o && onClose()}
      size="lg"
    >
      <Form label="Edit user" onSubmit={submit}>
        {update.isError && <Alert tone="danger">{errorMessage(update.error)}</Alert>}
        <FieldGrid>
          <Controller
            control={form.control}
            name="display_name"
            render={({ field }) => <TextField label="Display name" value={field.value} onChange={field.onChange} />}
          />
          <Controller
            control={form.control}
            name="email"
            render={({ field, fieldState }) => (
              <TextField
                label="Email"
                type="email"
                value={field.value}
                onChange={field.onChange}
                inputRef={field.ref}
                errorMessage={fieldState.error?.message}
              />
            )}
          />
          <Controller
            control={form.control}
            name="role"
            render={({ field }) => (
              <SelectField
                label="Role"
                description="Changing the role signs the user out."
                value={field.value}
                onChange={field.onChange}
                options={ROLE_OPTIONS}
              />
            )}
          />
          <Controller
            control={form.control}
            name="status"
            render={({ field }) => (
              <SelectField
                label="Status"
                value={field.value}
                onChange={field.onChange}
                options={[
                  { id: "active", label: "Active" },
                  { id: "pending", label: "Pending approval" },
                  { id: "disabled", label: "Disabled" },
                ]}
              />
            )}
          />
        </FieldGrid>
        <Controller
          control={form.control}
          name="zones"
          render={({ field, fieldState }) => (
            <TextAreaField
              label="Zones"
              description="One zone per line. Admins and Operators see every zone."
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
          name="account_ids"
          render={({ field }) => (
            <CheckboxGroupField
              label="Accounts"
              value={field.value}
              onChange={field.onChange}
              options={(accounts.data ?? []).map((a) => ({ id: String(a.id), label: a.name }))}
              columns={3}
            />
          )}
        />
        <DialogFooter>
          <Button onPress={onClose}>Cancel</Button>
          <Button type="submit" variant="primary" isPending={update.isPending}>
            Save user
          </Button>
        </DialogFooter>
      </Form>
    </Dialog>
  );
}

function ResetPasswordDialog({ user, onClose }: { user: User | null; onClose: () => void }) {
  const reset = useResetUserPassword();
  const [password, setPassword] = useState("");
  const [error, setError] = useState<string | undefined>();
  const close = () => {
    setPassword("");
    setError(undefined);
    reset.reset();
    onClose();
  };
  return (
    <Dialog
      title={`Reset password for ${user?.username ?? ""}`}
      isOpen={user !== null}
      onOpenChange={(o) => !o && close()}
      size="sm"
    >
      <Form
        label="Reset password"
        onSubmit={() => {
          if (password.length < MIN_PASSWORD) {
            setError(`Use at least ${MIN_PASSWORD} characters.`);
            return;
          }
          setError(undefined);
          if (user) {
            reset.mutate({ id: user.id, password }, { onSuccess: close });
          }
        }}
      >
        {reset.isError && <Alert tone="danger">{errorMessage(reset.error)}</Alert>}
        <TextField
          label="New password"
          type="password"
          value={password}
          onChange={setPassword}
          errorMessage={error}
          autoComplete="new-password"
        />
        <DialogFooter>
          <Button onPress={close}>Cancel</Button>
          <Button type="submit" variant="primary" isPending={reset.isPending}>
            Reset password
          </Button>
        </DialogFooter>
      </Form>
    </Dialog>
  );
}

const CONFIRM: Record<UserAction, { title: string; label: string; body: (u: User) => string }> = {
  "revoke-sessions": {
    title: "Sign out everywhere",
    label: "Revoke sessions",
    body: (u) => `End all of ${u.username}'s sessions? They will need to sign in again.`,
  },
  "reset-totp": {
    title: "Reset two-factor authentication",
    label: "Reset",
    body: (u) => `Remove the authenticator from ${u.username}? They can set it up again from their profile.`,
  },
  delete: {
    title: "Delete user",
    label: "Delete user",
    body: (u) => `Delete ${u.username}? Their API keys are disabled.`,
  },
};

function UsersPage() {
  const me = useMe();
  const users = useUsers();
  const update = useUpdateUser();
  const action = useUserAction();
  const [createOpen, setCreateOpen] = useState(false);
  const [editing, setEditing] = useState<User | null>(null);
  const [resetting, setResetting] = useState<User | null>(null);
  const [confirm, setConfirm] = useState<{ user: User; action: UserAction } | null>(null);

  const approve = (user: User) =>
    update.mutate({
      id: user.id,
      body: {
        display_name: user.display_name,
        email: user.email,
        role: user.role,
        status: "active",
        zones: user.zones,
        account_ids: user.account_ids,
      },
    });

  const onMenu = (user: User, key: string) => {
    if (key === "edit") {
      setEditing(user);
    } else if (key === "password") {
      setResetting(user);
    } else if (key === "revoke-sessions" || key === "reset-totp" || key === "delete") {
      action.reset();
      setConfirm({ user, action: key });
    }
  };

  return (
    <>
      <PageHeader
        title="Users"
        actions={
          <Button variant="primary" onPress={() => setCreateOpen(true)}>
            <Plus aria-hidden />
            Create local user
          </Button>
        }
      />
      {update.isError && editing === null && (
        <div className="mb-3">
          <Alert tone="danger">{errorMessage(update.error)}</Alert>
        </div>
      )}
      <Card flush>
        {users.isPending ? (
          <LoadingState />
        ) : users.isError ? (
          <ErrorState error={users.error} onRetry={() => void users.refetch()} />
        ) : users.data.length === 0 ? (
          <EmptyState title="No users" />
        ) : (
          <Table label="Users">
            <THead>
              <tr>
                <Th>Username</Th>
                <Th>Name</Th>
                <Th>Role</Th>
                <Th>Sign-in</Th>
                <Th>Status</Th>
                <Th>Two-factor</Th>
                <Th>Last sign-in</Th>
                <Th hidden>Actions</Th>
              </tr>
            </THead>
            <TBody>
              {users.data.map((user) => (
                <Tr key={user.id}>
                  <RowHeader>{user.username}</RowHeader>
                  <Td>{user.display_name}</Td>
                  <Td>{ROLE_LABELS[user.role]}</Td>
                  <Td>{user.auth_source === "oidc" ? "Single sign-on" : "Local"}</Td>
                  <Td>
                    <Badge tone={STATUS[user.status].tone}>{STATUS[user.status].label}</Badge>
                  </Td>
                  <Td>{user.totp_enabled ? "On" : "Off"}</Td>
                  <Td>{formatTime(user.last_login_at)}</Td>
                  <Td align="right">
                    <RowActions>
                      {user.status === "pending" && (
                        <Button size="sm" variant="primary" onPress={() => approve(user)}>
                          Approve
                        </Button>
                      )}
                      <MenuTrigger>
                        <Button variant="ghost" size="icon" aria-label={`Actions for ${user.username}`}>
                          <MoreHorizontal aria-hidden />
                        </Button>
                        <Menu label={`Actions for ${user.username}`} onAction={(key) => onMenu(user, key)}>
                          <MenuItem id="edit">Edit</MenuItem>
                          {user.auth_source === "local" ? <MenuItem id="password">Reset password</MenuItem> : null}
                          <MenuItem id="revoke-sessions">Revoke sessions</MenuItem>
                          {user.totp_enabled ? <MenuItem id="reset-totp">Reset two-factor</MenuItem> : null}
                          {user.id !== me.user.id ? (
                            <MenuItem id="delete" isDanger>
                              Delete
                            </MenuItem>
                          ) : null}
                        </Menu>
                      </MenuTrigger>
                    </RowActions>
                  </Td>
                </Tr>
              ))}
            </TBody>
          </Table>
        )}
      </Card>
      <CreateUserDialog isOpen={createOpen} onOpenChange={setCreateOpen} />
      <EditUserDialog user={editing} onClose={() => setEditing(null)} />
      <ResetPasswordDialog user={resetting} onClose={() => setResetting(null)} />
      {confirm && (
        <ConfirmDialog
          title={CONFIRM[confirm.action].title}
          isOpen
          onOpenChange={(o) => !o && setConfirm(null)}
          confirmLabel={CONFIRM[confirm.action].label}
          isPending={action.isPending}
          error={action.error}
          onConfirm={() =>
            action.mutate({ id: confirm.user.id, action: confirm.action }, { onSuccess: () => setConfirm(null) })
          }
        >
          <p>{CONFIRM[confirm.action].body(confirm.user)}</p>
        </ConfirmDialog>
      )}
    </>
  );
}
