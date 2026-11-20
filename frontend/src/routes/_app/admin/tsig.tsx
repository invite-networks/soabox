import { createFileRoute } from "@tanstack/react-router";
import { Eye, Plus, Trash2 } from "lucide-react";
import { useState } from "react";
import { errorMessage } from "../../../api/client";
import { useCreateTsigKey, useDeleteTsigKey, useTsigKey, useTsigKeys } from "../../../api/hooks/tsig";
import type { TsigKey } from "../../../api/types";
import { ConfirmDialog } from "../../../components/confirm-dialog";
import { RequirePermission } from "../../../components/forbidden";
import {
  Alert,
  Button,
  Card,
  CodeBlock,
  CopyButton,
  DetailList,
  Dialog,
  DialogFooter,
  EmptyState,
  ErrorState,
  Form,
  LoadingState,
  Mono,
  PageHeader,
  RowActions,
  RowHeader,
  SelectField,
  Table,
  TBody,
  Td,
  TextField,
  THead,
  Th,
  Tr,
} from "../../../components/ui";
import { isHostname } from "../../../lib/dns";

export const Route = createFileRoute("/_app/admin/tsig")({
  component: () => (
    <RequirePermission permission="manage_tsig" title="TSIG keys">
      <TsigPage />
    </RequirePermission>
  ),
});

const ALGORITHMS = ["hmac-sha256", "hmac-sha384", "hmac-sha512", "hmac-sha224", "hmac-sha1", "hmac-md5"].map((a) => ({
  id: a,
  label: a,
}));
const BASE64 = /^[A-Za-z0-9+/]+={0,2}$/;

function CreateDialog({
  isOpen,
  onOpenChange,
  onCreated,
}: {
  isOpen: boolean;
  onOpenChange: (o: boolean) => void;
  onCreated: (k: TsigKey) => void;
}) {
  const create = useCreateTsigKey();
  const [name, setName] = useState("");
  const [algorithm, setAlgorithm] = useState("hmac-sha256");
  const [secret, setSecret] = useState("");
  const [errors, setErrors] = useState<{ name?: string; key?: string }>({});
  return (
    <Dialog title="Create TSIG key" isOpen={isOpen} onOpenChange={onOpenChange}>
      <Form
        label="Create TSIG key"
        onSubmit={() => {
          const next: { name?: string; key?: string } = {};
          if (!isHostname(name.trim())) {
            next.name = "Enter a key name, for example transfer-key.";
          }
          if (secret && !BASE64.test(secret.trim())) {
            next.key = "Enter the secret in base64, or leave it empty to generate one.";
          }
          setErrors(next);
          if (next.name || next.key) {
            return;
          }
          create.mutate(
            { name: name.trim(), algorithm, ...(secret.trim() ? { key: secret.trim() } : {}) },
            {
              onSuccess: (key) => {
                onOpenChange(false);
                onCreated(key);
              },
            },
          );
        }}
      >
        {create.isError && <Alert tone="danger">{errorMessage(create.error)}</Alert>}
        <TextField label="Name" value={name} onChange={setName} errorMessage={errors.name} mono autoComplete="off" />
        <SelectField label="Algorithm" value={algorithm} onChange={setAlgorithm} options={ALGORITHMS} />
        <TextField
          label="Secret"
          description="Leave empty to have PowerDNS generate one."
          value={secret}
          onChange={setSecret}
          errorMessage={errors.key}
          mono
          autoComplete="off"
        />
        <DialogFooter>
          <Button onPress={() => onOpenChange(false)}>Cancel</Button>
          <Button type="submit" variant="primary" isPending={create.isPending}>
            Create key
          </Button>
        </DialogFooter>
      </Form>
    </Dialog>
  );
}

function ViewDialog({ id, onClose }: { id: string | null; onClose: () => void }) {
  const key = useTsigKey(id);
  return (
    <Dialog title="TSIG key" isOpen={id !== null} onOpenChange={(o) => !o && onClose()}>
      {key.isPending ? (
        <LoadingState />
      ) : key.isError ? (
        <ErrorState error={key.error} />
      ) : (
        <div className="flex flex-col gap-3">
          <DetailList
            items={[
              { label: "Name", value: <Mono>{key.data.name}</Mono> },
              { label: "Algorithm", value: <Mono>{key.data.algorithm}</Mono> },
            ]}
          />
          <CodeBlock label="Secret">{key.data.key}</CodeBlock>
          <div>
            <CopyButton text={key.data.key ?? ""} label="Copy secret" />
          </div>
        </div>
      )}
    </Dialog>
  );
}

function TsigPage() {
  const keys = useTsigKeys();
  const remove = useDeleteTsigKey();
  const [createOpen, setCreateOpen] = useState(false);
  const [viewing, setViewing] = useState<string | null>(null);
  const [deleting, setDeleting] = useState<TsigKey | null>(null);

  return (
    <>
      <PageHeader
        title="TSIG keys"
        description="Shared secrets for authenticating zone transfers and updates."
        actions={
          <Button variant="primary" onPress={() => setCreateOpen(true)}>
            <Plus aria-hidden />
            Create TSIG key
          </Button>
        }
      />
      <Card flush>
        {keys.isPending ? (
          <LoadingState />
        ) : keys.isError ? (
          <ErrorState error={keys.error} onRetry={() => void keys.refetch()} />
        ) : keys.data.length === 0 ? (
          <EmptyState title="No TSIG keys" />
        ) : (
          <Table label="TSIG keys">
            <THead>
              <tr>
                <Th>Name</Th>
                <Th>Algorithm</Th>
                <Th hidden>Actions</Th>
              </tr>
            </THead>
            <TBody>
              {keys.data.map((k) => (
                <Tr key={k.id}>
                  <RowHeader mono>{k.name}</RowHeader>
                  <Td mono>{k.algorithm}</Td>
                  <Td align="right">
                    <RowActions>
                      <Button
                        variant="ghost"
                        size="icon"
                        aria-label={`View ${k.name}`}
                        onPress={() => setViewing(k.id)}
                      >
                        <Eye aria-hidden />
                      </Button>
                      <Button
                        variant="ghost"
                        size="icon"
                        aria-label={`Delete ${k.name}`}
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
      <CreateDialog isOpen={createOpen} onOpenChange={setCreateOpen} onCreated={(k) => setViewing(k.id)} />
      <ViewDialog id={viewing} onClose={() => setViewing(null)} />
      <ConfirmDialog
        title="Delete TSIG key"
        isOpen={deleting !== null}
        onOpenChange={(o) => !o && setDeleting(null)}
        confirmLabel="Delete key"
        isPending={remove.isPending}
        error={remove.error}
        onConfirm={() => deleting && remove.mutate(deleting.id, { onSuccess: () => setDeleting(null) })}
      >
        <p>Delete {deleting?.name}? Transfers that use it will fail.</p>
      </ConfirmDialog>
    </>
  );
}
