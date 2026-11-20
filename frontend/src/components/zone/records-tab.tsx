import { Pencil, Plus, RotateCcw, Trash2 } from "lucide-react";
import { useMemo, useState } from "react";
import { errorMessage } from "../../api/client";
import { usePatchRrSets } from "../../api/hooks/zones";
import type { RrSet, Zone } from "../../api/types";
import { EDIT_ONLY_TYPES, toRelativeName } from "../../lib/dns";
import { formatTtl } from "../../lib/format";
import {
  type ChangeStatus,
  type PendingChanges,
  rrKey,
  toPatchRequest,
  withChange,
  withoutChange,
  workingRrSets,
} from "../../lib/pending";
import { DiffView } from "../diff-view";
import {
  Alert,
  Badge,
  Button,
  Card,
  Dialog,
  DialogFooter,
  EmptyState,
  RowActions,
  RowHeader,
  SearchField,
  Table,
  TBody,
  Td,
  THead,
  Th,
  type Tone,
  Tr,
} from "../ui";
import { RecordContent } from "./record-value-fields";
import { RrSetDialog } from "./rrset-dialog";
import { RecordType } from "./zone-labels";

const STATUS: Record<ChangeStatus, { label: string; tone: Tone }> = {
  added: { label: "Added", tone: "success" },
  changed: { label: "Changed", tone: "warning" },
  deleted: { label: "Deleted", tone: "danger" },
};

/** The record editor. Edits collect as pending changes and are applied in one PATCH. */
export function RecordsTab({ zone }: { zone: Zone }) {
  // The backend reports the types it accepts in this zone, so every user sees the same list.
  const allowedTypes = zone.allowed_record_types.filter((t) => !EDIT_ONLY_TYPES.has(t));
  const patch = usePatchRrSets(zone.id);
  const [changes, setChanges] = useState<PendingChanges>(new Map());
  const [filter, setFilter] = useState("");
  const [dialog, setDialog] = useState<{ open: boolean; editing: RrSet | null }>({ open: false, editing: null });
  const [reviewOpen, setReviewOpen] = useState(false);
  const [applied, setApplied] = useState(false);

  const rows = useMemo(() => workingRrSets(zone.rrsets, changes), [zone.rrsets, changes]);
  const visible = useMemo(() => {
    const q = filter.trim().toLowerCase();
    if (!q) {
      return rows;
    }
    return rows.filter(
      ({ rrset }) =>
        rrset.name.toLowerCase().includes(q) ||
        rrset.type.toLowerCase() === q ||
        rrset.records.some((r) => r.content.toLowerCase().includes(q)),
    );
  }, [rows, filter]);
  const existing = rows.filter((r) => r.status !== "deleted").map((r) => r.rrset);

  const original = (rrset: RrSet) =>
    rows.find((r) => rrKey(r.rrset.name, r.rrset.type) === rrKey(rrset.name, rrset.type))?.original ?? null;

  const save = (after: RrSet) => {
    setApplied(false);
    const key = rrKey(after.name, after.type);
    const current = rows.find((r) => rrKey(r.rrset.name, r.rrset.type) === key && r.status !== "deleted");
    let next = after;
    if (dialog.editing === null && current) {
      // Adding to an existing RRset appends the new values to it.
      const contents = new Set(current.rrset.records.map((r) => r.content));
      next = {
        ...current.rrset,
        ttl: after.ttl,
        records: [...current.rrset.records, ...after.records.filter((r) => !contents.has(r.content))],
      };
    }
    setChanges((prev) => withChange(prev, original(after), next));
  };

  const remove = (rrset: RrSet) => {
    setApplied(false);
    const before = original(rrset);
    setChanges((prev) =>
      before ? withChange(prev, before, null) : withoutChange(prev, rrKey(rrset.name, rrset.type)),
    );
  };

  const undo = (rrset: RrSet) => setChanges((prev) => withoutChange(prev, rrKey(rrset.name, rrset.type)));

  const apply = () => {
    patch.mutate(toPatchRequest(changes), {
      onSuccess: () => {
        setChanges(new Map());
        setReviewOpen(false);
        setApplied(true);
      },
    });
  };

  const pending = [...changes.values()];

  return (
    <div className="flex flex-col gap-4">
      {applied && <Alert tone="success">Changes applied.</Alert>}
      {pending.length > 0 && (
        <section
          aria-label="Pending changes"
          className="flex flex-wrap items-center justify-between gap-3 rounded-lg border border-warning/50 bg-warning-soft px-4 py-3"
        >
          <div className="text-sm text-fg">
            <p className="font-medium">
              {pending.length} pending {pending.length === 1 ? "change" : "changes"}
            </p>
            <p className="text-fg-muted">Nothing is saved until you review and apply.</p>
          </div>
          <div className="flex gap-2">
            <Button onPress={() => setChanges(new Map())}>Discard all</Button>
            <Button variant="primary" onPress={() => setReviewOpen(true)}>
              Review and apply
            </Button>
          </div>
        </section>
      )}
      <Card flush>
        <div className="flex flex-wrap items-end justify-between gap-3 border-b border-border p-3">
          <div className="flex w-full max-w-md items-center gap-3">
            <div className="flex-1">
              <SearchField
                label="Filter records"
                hideLabel
                placeholder="Filter records"
                value={filter}
                onChange={setFilter}
              />
            </div>
            <span className="text-xs whitespace-nowrap text-fg-muted">
              {visible.length === rows.length ? `${rows.length} RRsets` : `${visible.length} of ${rows.length} RRsets`}
            </span>
          </div>
          <div className="flex items-center gap-3">
            {allowedTypes.length === 0 && (
              <span className="text-sm text-fg-muted">No record types are allowed in this zone.</span>
            )}
            <Button
              variant="primary"
              isDisabled={allowedTypes.length === 0}
              onPress={() => setDialog({ open: true, editing: null })}
            >
              <Plus aria-hidden />
              Add record
            </Button>
          </div>
        </div>
        {visible.length === 0 ? (
          <EmptyState title={filter ? "No records match the filter" : "This zone has no records"} />
        ) : (
          <Table label="Records">
            <THead>
              <tr>
                <Th>Name</Th>
                <Th>Type</Th>
                <Th align="right">TTL</Th>
                <Th>Content</Th>
                <Th hidden>Actions</Th>
              </tr>
            </THead>
            <TBody>
              {visible.map(({ rrset, status }) => {
                const relative = toRelativeName(rrset.name, zone.name);
                const label = `${relative} ${rrset.type}`;
                return (
                  <Tr key={rrKey(rrset.name, rrset.type)} muted={status === "deleted"}>
                    <RowHeader mono>
                      <span className={relative === "@" ? "text-fg-muted" : ""}>{relative}</span>
                    </RowHeader>
                    <Td>
                      <RecordType type={rrset.type} />
                    </Td>
                    <Td align="right" mono>
                      <span title={`${rrset.ttl} seconds`} className="text-fg-muted">
                        {formatTtl(rrset.ttl)}
                      </span>
                    </Td>
                    <Td mono wrap>
                      <ul className={status === "deleted" ? "line-through" : ""}>
                        {rrset.records.map((record) => (
                          <li key={record.content} className="flex flex-wrap items-center gap-2">
                            <span className={record.disabled ? "text-fg-muted" : ""}>
                              <RecordContent type={rrset.type} content={record.content} zone={zone.name} />
                            </span>
                            {record.disabled && <Badge>Disabled</Badge>}
                          </li>
                        ))}
                      </ul>
                    </Td>
                    <Td align="right">
                      <RowActions>
                        {status && <Badge tone={STATUS[status].tone}>{STATUS[status].label}</Badge>}
                        {status ? (
                          <Button
                            variant="ghost"
                            size="icon"
                            aria-label={`Undo change to ${label}`}
                            onPress={() => undo(rrset)}
                          >
                            <RotateCcw aria-hidden />
                          </Button>
                        ) : null}
                        {status !== "deleted" && (
                          <Button
                            variant="ghost"
                            size="icon"
                            aria-label={`Edit ${label}`}
                            onPress={() => setDialog({ open: true, editing: rrset })}
                          >
                            <Pencil aria-hidden />
                          </Button>
                        )}
                        {status !== "deleted" && !EDIT_ONLY_TYPES.has(rrset.type) && (
                          <Button
                            variant="ghost"
                            size="icon"
                            aria-label={`Delete ${label}`}
                            onPress={() => remove(rrset)}
                          >
                            <Trash2 aria-hidden />
                          </Button>
                        )}
                      </RowActions>
                    </Td>
                  </Tr>
                );
              })}
            </TBody>
          </Table>
        )}
      </Card>

      <RrSetDialog
        zoneName={zone.name}
        isOpen={dialog.open}
        onOpenChange={(open) => setDialog((d) => ({ ...d, open }))}
        editing={dialog.editing}
        allowedTypes={allowedTypes}
        existing={existing}
        onSave={save}
      />

      <Dialog
        title="Review pending changes"
        isOpen={reviewOpen}
        onOpenChange={(open) => {
          setReviewOpen(open);
          if (!open) {
            patch.reset();
          }
        }}
        size="xl"
      >
        <div className="flex flex-col gap-3">
          <p className="text-sm text-fg-muted">
            These changes are sent to PowerDNS together as one update and recorded in history.
          </p>
          <DiffView
            diff={pending.map((c) => ({
              name: toRelativeName(c.name, zone.name),
              type: c.type,
              before: c.before,
              after: c.after,
            }))}
          />
          {patch.isError && (
            <Alert tone="danger" title="The changes were not applied">
              {errorMessage(patch.error)}
            </Alert>
          )}
        </div>
        <DialogFooter>
          <Button onPress={() => setReviewOpen(false)}>Keep editing</Button>
          <Button variant="primary" isPending={patch.isPending} onPress={apply}>
            Apply {pending.length} {pending.length === 1 ? "change" : "changes"}
          </Button>
        </DialogFooter>
      </Dialog>
    </div>
  );
}
