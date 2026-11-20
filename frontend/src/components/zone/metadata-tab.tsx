import { Pencil, Plus, Trash2 } from "lucide-react";
import { useState } from "react";
import { errorMessage } from "../../api/client";
import { useDeleteMetadata, useMetadata, usePutMetadata } from "../../api/hooks/dnssec";
import type { Metadata, Zone } from "../../api/types";
import { joinList, splitList } from "../../lib/format";
import { ConfirmDialog } from "../confirm-dialog";
import {
  Alert,
  Button,
  Card,
  Dialog,
  DialogFooter,
  EmptyState,
  ErrorState,
  Form,
  LoadingState,
  RowActions,
  RowHeader,
  Table,
  TBody,
  Td,
  TextAreaField,
  TextField,
  THead,
  Th,
  Tr,
} from "../ui";

function MetadataDialog({
  zoneId,
  editing,
  isOpen,
  onOpenChange,
}: {
  zoneId: string;
  editing: Metadata | null;
  isOpen: boolean;
  onOpenChange: (o: boolean) => void;
}) {
  const put = usePutMetadata(zoneId);
  const [kind, setKind] = useState(editing?.kind ?? "");
  const [values, setValues] = useState(joinList(editing?.metadata ?? []));
  const [kindError, setKindError] = useState<string | undefined>();
  return (
    <Dialog title={editing ? `Edit ${editing.kind}` : "Add metadata"} isOpen={isOpen} onOpenChange={onOpenChange}>
      <Form
        label="Metadata"
        onSubmit={() => {
          const k = kind.trim();
          if (!/^[A-Za-z0-9-]+$/.test(k)) {
            setKindError("Enter a metadata kind, for example ALLOW-AXFR-FROM.");
            return;
          }
          setKindError(undefined);
          put.mutate({ kind: k, metadata: splitList(values) }, { onSuccess: () => onOpenChange(false) });
        }}
      >
        {put.isError && <Alert tone="danger">{errorMessage(put.error)}</Alert>}
        <TextField
          label="Kind"
          value={kind}
          onChange={setKind}
          errorMessage={kindError}
          isReadOnly={editing !== null}
          mono
          autoComplete="off"
        />
        <TextAreaField
          label="Values"
          description="One value per line."
          value={values}
          onChange={setValues}
          mono
          rows={4}
        />
        <DialogFooter>
          <Button onPress={() => onOpenChange(false)}>Cancel</Button>
          <Button type="submit" variant="primary" isPending={put.isPending}>
            Save
          </Button>
        </DialogFooter>
      </Form>
    </Dialog>
  );
}

/** Zone metadata view and edit. */
export function MetadataTab({ zone }: { zone: Zone }) {
  const metadata = useMetadata(zone.id);
  const remove = useDeleteMetadata(zone.id);
  const [dialog, setDialog] = useState<{ open: boolean; editing: Metadata | null; seq: number }>({
    open: false,
    editing: null,
    seq: 0,
  });
  const [deleting, setDeleting] = useState<string | null>(null);
  const open = (editing: Metadata | null) => setDialog((d) => ({ open: true, editing, seq: d.seq + 1 }));

  return (
    <Card
      title="Metadata"
      flush
      actions={
        <Button size="sm" onPress={() => open(null)}>
          <Plus aria-hidden />
          Add metadata
        </Button>
      }
    >
      {metadata.isPending ? (
        <LoadingState />
      ) : metadata.isError ? (
        <ErrorState error={metadata.error} onRetry={() => void metadata.refetch()} />
      ) : metadata.data.length === 0 ? (
        <EmptyState title="No metadata" />
      ) : (
        <Table label="Metadata">
          <THead>
            <tr>
              <Th>Kind</Th>
              <Th>Values</Th>
              <Th hidden>Actions</Th>
            </tr>
          </THead>
          <TBody>
            {metadata.data.map((item) => (
              <Tr key={item.kind}>
                <RowHeader mono>{item.kind}</RowHeader>
                <Td mono wrap>
                  {item.metadata.join(", ")}
                </Td>
                <Td align="right">
                  <RowActions>
                    <Button variant="ghost" size="icon" aria-label={`Edit ${item.kind}`} onPress={() => open(item)}>
                      <Pencil aria-hidden />
                    </Button>
                    <Button
                      variant="ghost"
                      size="icon"
                      aria-label={`Delete ${item.kind}`}
                      onPress={() => setDeleting(item.kind)}
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
      <MetadataDialog
        key={dialog.seq}
        zoneId={zone.id}
        editing={dialog.editing}
        isOpen={dialog.open}
        onOpenChange={(o) => setDialog((d) => ({ ...d, open: o }))}
      />
      <ConfirmDialog
        title="Delete metadata"
        isOpen={deleting !== null}
        onOpenChange={(o) => {
          if (!o) {
            setDeleting(null);
          }
        }}
        confirmLabel="Delete"
        isPending={remove.isPending}
        error={remove.error}
        onConfirm={() => {
          if (deleting) {
            remove.mutate(deleting, { onSuccess: () => setDeleting(null) });
          }
        }}
      >
        <p>Delete the {deleting} metadata from this zone?</p>
      </ConfirmDialog>
    </Card>
  );
}
