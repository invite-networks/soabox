import { Plus, ShieldCheck, ShieldOff, Trash2 } from "lucide-react";
import { useState } from "react";
import { errorMessage } from "../../api/client";
import {
  useCreateCryptoKey,
  useCryptoKeys,
  useDeleteCryptoKey,
  useSetDnssec,
  useUpdateCryptoKey,
} from "../../api/hooks/dnssec";
import type { CryptoKey, KeyType, Zone } from "../../api/types";
import { ConfirmDialog } from "../confirm-dialog";
import {
  Alert,
  Badge,
  Button,
  Card,
  CodeBlock,
  CopyButton,
  Dialog,
  DialogFooter,
  EmptyState,
  ErrorState,
  FieldGrid,
  Form,
  LoadingState,
  RowActions,
  SelectField,
  Table,
  TBody,
  Td,
  TextField,
  THead,
  Th,
  Tr,
} from "../ui";

const ALGORITHMS = [
  { id: "default", label: "Server default" },
  { id: "ecdsap256sha256", label: "ECDSAP256SHA256" },
  { id: "ecdsap384sha384", label: "ECDSAP384SHA384" },
  { id: "ed25519", label: "ED25519" },
  { id: "ed448", label: "ED448" },
  { id: "rsasha256", label: "RSASHA256" },
  { id: "rsasha512", label: "RSASHA512" },
];

function AddKeyDialog({
  zoneId,
  isOpen,
  onOpenChange,
}: {
  zoneId: string;
  isOpen: boolean;
  onOpenChange: (o: boolean) => void;
}) {
  const create = useCreateCryptoKey(zoneId);
  const [keytype, setKeytype] = useState<KeyType>("csk");
  const [algorithm, setAlgorithm] = useState("default");
  const [bits, setBits] = useState("");
  const [active, setActive] = useState("active");
  const bitsError = bits !== "" && !/^\d+$/.test(bits) ? "Enter a number of bits." : undefined;

  return (
    <Dialog title="Add key" isOpen={isOpen} onOpenChange={onOpenChange}>
      <Form
        label="Add key"
        onSubmit={() => {
          if (bitsError) {
            return;
          }
          create.mutate(
            {
              keytype,
              active: active === "active",
              ...(algorithm !== "default" ? { algorithm } : {}),
              ...(bits ? { bits: Number(bits) } : {}),
            },
            { onSuccess: () => onOpenChange(false) },
          );
        }}
      >
        {create.isError && <Alert tone="danger">{errorMessage(create.error)}</Alert>}
        <FieldGrid>
          <SelectField
            label="Key type"
            value={keytype}
            onChange={(v) => setKeytype(v as KeyType)}
            options={[
              { id: "csk", label: "CSK (combined)" },
              { id: "ksk", label: "KSK (key signing)" },
              { id: "zsk", label: "ZSK (zone signing)" },
            ]}
          />
          <SelectField
            label="State"
            value={active}
            onChange={setActive}
            options={[
              { id: "active", label: "Active" },
              { id: "inactive", label: "Inactive" },
            ]}
          />
          <SelectField label="Algorithm" value={algorithm} onChange={setAlgorithm} options={ALGORITHMS} />
          <TextField
            label="Bits"
            description="Leave empty for the algorithm default."
            value={bits}
            onChange={setBits}
            errorMessage={bitsError}
            inputMode="numeric"
            mono
          />
        </FieldGrid>
        <DialogFooter>
          <Button onPress={() => onOpenChange(false)}>Cancel</Button>
          <Button type="submit" variant="primary" isPending={create.isPending}>
            Add key
          </Button>
        </DialogFooter>
      </Form>
    </Dialog>
  );
}

function KeyRow({ zoneId, cryptoKey }: { zoneId: string; cryptoKey: CryptoKey }) {
  const update = useUpdateCryptoKey(zoneId);
  const remove = useDeleteCryptoKey(zoneId);
  const [confirm, setConfirm] = useState(false);
  const label = `${cryptoKey.keytype.toUpperCase()} key ${cryptoKey.id}`;
  return (
    <Tr>
      <Td mono>{cryptoKey.id}</Td>
      <Td>{cryptoKey.keytype.toUpperCase()}</Td>
      <Td mono>{cryptoKey.algorithm}</Td>
      <Td align="right" mono>
        {cryptoKey.bits}
      </Td>
      <Td>
        {cryptoKey.active ? <Badge tone="success">Active</Badge> : <Badge>Inactive</Badge>}{" "}
        {cryptoKey.published ? <Badge tone="accent">Published</Badge> : <Badge>Not published</Badge>}
      </Td>
      <Td align="right">
        <RowActions>
          <Button
            size="sm"
            isPending={update.isPending}
            onPress={() =>
              update.mutate({
                keyId: cryptoKey.id,
                body: { active: !cryptoKey.active, published: cryptoKey.published },
              })
            }
          >
            {cryptoKey.active ? "Deactivate" : "Activate"}
          </Button>
          <Button
            size="sm"
            onPress={() =>
              update.mutate({
                keyId: cryptoKey.id,
                body: { active: cryptoKey.active, published: !cryptoKey.published },
              })
            }
          >
            {cryptoKey.published ? "Unpublish" : "Publish"}
          </Button>
          <Button variant="ghost" size="icon" aria-label={`Remove ${label}`} onPress={() => setConfirm(true)}>
            <Trash2 aria-hidden />
          </Button>
        </RowActions>
        {update.isError && <p className="mt-1 text-xs text-danger">{errorMessage(update.error)}</p>}
        <ConfirmDialog
          title="Remove key"
          isOpen={confirm}
          onOpenChange={setConfirm}
          confirmLabel="Remove key"
          isPending={remove.isPending}
          error={remove.error}
          onConfirm={() => remove.mutate(cryptoKey.id, { onSuccess: () => setConfirm(false) })}
        >
          <p>
            Remove {label}? If it is the only active key, the zone will fail validation for resolvers until the DS
            records at the registrar are updated.
          </p>
        </ConfirmDialog>
      </Td>
    </Tr>
  );
}

/** DNSSEC signing state, keys, and DS records. */
export function DnssecTab({ zone }: { zone: Zone }) {
  const keys = useCryptoKeys(zone.id, zone.dnssec);
  const setDnssec = useSetDnssec(zone.id);
  const [addOpen, setAddOpen] = useState(false);
  const [confirmDisable, setConfirmDisable] = useState(false);
  const ds = (keys.data ?? []).flatMap((k) => k.ds);

  return (
    <div className="flex flex-col gap-4">
      <Card title="Signing">
        <div className="flex flex-wrap items-center justify-between gap-3">
          <div className="flex items-center gap-2 text-sm">
            {zone.dnssec ? (
              <Badge tone="success">
                <ShieldCheck aria-hidden />
                Signed
              </Badge>
            ) : (
              <Badge>
                <ShieldOff aria-hidden />
                Unsigned
              </Badge>
            )}
            {zone.nsec3param && <span className="font-mono text-xs text-fg-muted">NSEC3 {zone.nsec3param}</span>}
          </div>
          {zone.dnssec ? (
            <Button variant="danger" onPress={() => setConfirmDisable(true)}>
              Disable DNSSEC
            </Button>
          ) : (
            <Button
              variant="primary"
              isPending={setDnssec.isPending}
              onPress={() => setDnssec.mutate({ enabled: true })}
            >
              Enable DNSSEC
            </Button>
          )}
        </div>
        {setDnssec.isError && !confirmDisable && (
          <div className="mt-3">
            <Alert tone="danger">{errorMessage(setDnssec.error)}</Alert>
          </div>
        )}
      </Card>

      {zone.dnssec && (
        <>
          <Card title="DS records">
            {keys.isPending ? (
              <LoadingState />
            ) : ds.length === 0 ? (
              <p className="text-sm text-fg-muted">No DS records. Add an active KSK or CSK.</p>
            ) : (
              <div className="flex flex-col gap-2">
                <p className="text-sm text-fg-muted">Give these to your registrar to complete the chain of trust.</p>
                <CodeBlock label="DS records">{ds.join("\n")}</CodeBlock>
                <div>
                  <CopyButton text={ds.join("\n")} label="Copy DS records" />
                </div>
              </div>
            )}
          </Card>
          <Card
            title="Keys"
            flush
            actions={
              <Button size="sm" onPress={() => setAddOpen(true)}>
                <Plus aria-hidden />
                Add key
              </Button>
            }
          >
            {keys.isPending ? (
              <LoadingState label="Loading keys" />
            ) : keys.isError ? (
              <ErrorState error={keys.error} onRetry={() => void keys.refetch()} />
            ) : keys.data.length === 0 ? (
              <EmptyState title="No keys" />
            ) : (
              <Table label="DNSSEC keys">
                <THead>
                  <tr>
                    <Th>ID</Th>
                    <Th>Type</Th>
                    <Th>Algorithm</Th>
                    <Th align="right">Bits</Th>
                    <Th>State</Th>
                    <Th hidden>Actions</Th>
                  </tr>
                </THead>
                <TBody>
                  {keys.data.map((k) => (
                    <KeyRow key={k.id} zoneId={zone.id} cryptoKey={k} />
                  ))}
                </TBody>
              </Table>
            )}
          </Card>
        </>
      )}

      <AddKeyDialog zoneId={zone.id} isOpen={addOpen} onOpenChange={setAddOpen} />
      <ConfirmDialog
        title="Disable DNSSEC"
        isOpen={confirmDisable}
        onOpenChange={setConfirmDisable}
        confirmLabel="Disable DNSSEC"
        isPending={setDnssec.isPending}
        error={setDnssec.error}
        onConfirm={() => setDnssec.mutate({ enabled: false }, { onSuccess: () => setConfirmDisable(false) })}
      >
        <p>
          Remove the DS records at your registrar first. Disabling signing while DS records are published makes the zone
          fail validation.
        </p>
      </ConfirmDialog>
    </div>
  );
}
