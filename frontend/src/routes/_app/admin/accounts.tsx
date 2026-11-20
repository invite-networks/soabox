import { createFileRoute } from "@tanstack/react-router";
import { Pencil, Plus, Trash2 } from "lucide-react";
import { useState } from "react";
import { errorMessage } from "../../../api/client";
import { useAccounts, useCreateAccount, useDeleteAccount, useUpdateAccount } from "../../../api/hooks/accounts";
import { useUsers } from "../../../api/hooks/users";
import type { Account } from "../../../api/types";
import { ConfirmDialog } from "../../../components/confirm-dialog";
import { RequirePermission } from "../../../components/forbidden";
import {
  Alert,
  Button,
  Card,
  CheckboxGroupField,
  Dialog,
  DialogFooter,
  EmptyState,
  ErrorState,
  Form,
  LoadingState,
  PageHeader,
  RowActions,
  RowHeader,
  Table,
  TBody,
  Td,
  TextField,
  THead,
  Th,
  Tr,
} from "../../../components/ui";
import { ZonePickerField } from "../../../components/zone-picker";

export const Route = createFileRoute("/_app/admin/accounts")({
  component: () => (
    <RequirePermission permission="manage_users" title="Accounts">
      <AccountsPage />
    </RequirePermission>
  ),
});

function AccountDialog({
  editing,
  isOpen,
  onOpenChange,
}: {
  editing: Account | null;
  isOpen: boolean;
  onOpenChange: (o: boolean) => void;
}) {
  const create = useCreateAccount();
  const update = useUpdateAccount();
  const users = useUsers(isOpen && editing !== null);
  const mutation = editing ? update : create;
  const [name, setName] = useState(editing?.name ?? "");
  const [description, setDescription] = useState(editing?.description ?? "");
  const [zones, setZones] = useState<string[]>(editing?.zones ?? []);
  const [members, setMembers] = useState<string[]>((editing?.user_ids ?? []).map(String));
  const [nameError, setNameError] = useState<string | undefined>();

  return (
    <Dialog
      title={editing ? `Edit ${editing.name}` : "Create account"}
      isOpen={isOpen}
      onOpenChange={onOpenChange}
      size="lg"
    >
      <Form
        label="Account"
        onSubmit={() => {
          if (!name.trim()) {
            setNameError("Enter a name.");
            return;
          }
          setNameError(undefined);
          if (editing) {
            update.mutate(
              {
                id: editing.id,
                body: {
                  name: name.trim(),
                  description,
                  zones,
                  user_ids: members.map(Number),
                },
              },
              { onSuccess: () => onOpenChange(false) },
            );
          } else {
            create.mutate({ name: name.trim(), description }, { onSuccess: () => onOpenChange(false) });
          }
        }}
      >
        {mutation.isError && <Alert tone="danger">{errorMessage(mutation.error)}</Alert>}
        <TextField label="Name" value={name} onChange={setName} errorMessage={nameError} />
        <TextField label="Description" value={description} onChange={setDescription} />
        {editing && (
          <>
            <ZonePickerField value={zones} onChange={setZones} />
            <CheckboxGroupField
              label="Members"
              value={members}
              onChange={setMembers}
              options={(users.data ?? []).map((u) => ({ id: String(u.id), label: u.display_name || u.username }))}
              columns={3}
            />
          </>
        )}
        <DialogFooter>
          <Button onPress={() => onOpenChange(false)}>Cancel</Button>
          <Button type="submit" variant="primary" isPending={mutation.isPending}>
            {editing ? "Save account" : "Create account"}
          </Button>
        </DialogFooter>
      </Form>
    </Dialog>
  );
}

function AccountsPage() {
  const accounts = useAccounts();
  const remove = useDeleteAccount();
  const [dialog, setDialog] = useState<{ open: boolean; editing: Account | null; seq: number }>({
    open: false,
    editing: null,
    seq: 0,
  });
  const [deleting, setDeleting] = useState<Account | null>(null);
  const open = (editing: Account | null) => setDialog((d) => ({ open: true, editing, seq: d.seq + 1 }));

  return (
    <>
      <PageHeader
        title="Accounts"
        description="Accounts group zones and users. Members reach every zone of their accounts."
        actions={
          <Button variant="primary" onPress={() => open(null)}>
            <Plus aria-hidden />
            Create account
          </Button>
        }
      />
      <Card flush>
        {accounts.isPending ? (
          <LoadingState />
        ) : accounts.isError ? (
          <ErrorState error={accounts.error} onRetry={() => void accounts.refetch()} />
        ) : accounts.data.length === 0 ? (
          <EmptyState title="No accounts yet" />
        ) : (
          <Table label="Accounts">
            <THead>
              <tr>
                <Th>Name</Th>
                <Th>Description</Th>
                <Th align="right">Zones</Th>
                <Th align="right">Members</Th>
                <Th hidden>Actions</Th>
              </tr>
            </THead>
            <TBody>
              {accounts.data.map((a) => (
                <Tr key={a.id}>
                  <RowHeader>{a.name}</RowHeader>
                  <Td wrap>{a.description}</Td>
                  <Td align="right">{a.zones.length}</Td>
                  <Td align="right">{a.user_ids.length}</Td>
                  <Td align="right">
                    <RowActions>
                      <Button variant="ghost" size="icon" aria-label={`Edit ${a.name}`} onPress={() => open(a)}>
                        <Pencil aria-hidden />
                      </Button>
                      <Button
                        variant="ghost"
                        size="icon"
                        aria-label={`Delete ${a.name}`}
                        onPress={() => setDeleting(a)}
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
      <AccountDialog
        key={dialog.seq}
        editing={dialog.editing}
        isOpen={dialog.open}
        onOpenChange={(o) => setDialog((d) => ({ ...d, open: o }))}
      />
      <ConfirmDialog
        title="Delete account"
        isOpen={deleting !== null}
        onOpenChange={(o) => !o && setDeleting(null)}
        confirmLabel="Delete account"
        isPending={remove.isPending}
        error={remove.error}
        onConfirm={() => deleting && remove.mutate(deleting.id, { onSuccess: () => setDeleting(null) })}
      >
        <p>Delete the account {deleting?.name}? Its zones and users are kept but lose this grouping.</p>
      </ConfirmDialog>
    </>
  );
}
