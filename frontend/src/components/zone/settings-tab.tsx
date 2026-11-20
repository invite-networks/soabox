import { useNavigate } from "@tanstack/react-router";
import { Download, RefreshCw, Send, Wrench } from "lucide-react";
import { useState } from "react";
import { errorMessage } from "../../api/client";
import { useAccounts } from "../../api/hooks/accounts";
import { useDeleteZone, useUpdateZone, useZoneAction, useZoneExport, type ZoneAction } from "../../api/hooks/zones";
import type { Zone, ZoneKind } from "../../api/types";
import { isIPv4, isIPv6 } from "../../lib/dns";
import { joinList, splitList } from "../../lib/format";
import { useMe } from "../../lib/me-context";
import { can } from "../../lib/permissions";
import { ConfirmDialog } from "../confirm-dialog";
import { SOA_EDIT_API_OPTIONS } from "../create-zone-dialog";
import { Alert, Button, Card, FieldGrid, Form, SelectField, Switch, TextAreaField } from "../ui";

const NONE = "none";

function downloadText(filename: string, text: string) {
  const url = URL.createObjectURL(new Blob([text], { type: "text/plain" }));
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = filename;
  anchor.click();
  URL.revokeObjectURL(url);
}

function ZoneForm({ zone }: { zone: Zone }) {
  const update = useUpdateZone(zone.id);
  const accounts = useAccounts();
  const [kind, setKind] = useState<ZoneKind>(zone.kind);
  const [masters, setMasters] = useState(joinList(zone.masters));
  const [account, setAccount] = useState(zone.account ? String(zone.account.id) : NONE);
  const [soaEditApi, setSoaEditApi] = useState(zone.soa_edit_api || "DEFAULT");
  const [rectify, setRectify] = useState(zone.api_rectify);
  const [mastersError, setMastersError] = useState<string | undefined>();
  const [saved, setSaved] = useState(false);

  const accountOptions = [
    { id: NONE, label: "No account" },
    ...(accounts.data ?? []).map((a) => ({ id: String(a.id), label: a.name })),
  ];
  if (zone.account && !accountOptions.some((o) => o.id === String(zone.account?.id))) {
    accountOptions.push({ id: String(zone.account.id), label: zone.account.name });
  }

  return (
    <Form
      label="Zone settings"
      onSubmit={() => {
        const list = splitList(masters);
        const bad = list.find((m) => !isIPv4(m.replace(/:\d+$/, "")) && !isIPv6(m));
        if (kind === "Slave" && list.length === 0) {
          setMastersError("Enter at least one primary server address.");
          return;
        }
        if (bad) {
          setMastersError(`${bad} is not an IP address.`);
          return;
        }
        setMastersError(undefined);
        setSaved(false);
        update.mutate(
          {
            kind,
            masters: kind === "Slave" ? list : [],
            account_id: account === NONE ? null : Number(account),
            soa_edit_api: soaEditApi,
            api_rectify: rectify,
          },
          { onSuccess: () => setSaved(true) },
        );
      }}
    >
      {update.isError && <Alert tone="danger">{errorMessage(update.error)}</Alert>}
      {saved && <Alert tone="success">Zone settings saved.</Alert>}
      <FieldGrid columns={3}>
        <SelectField
          label="Kind"
          value={kind}
          onChange={(v) => setKind(v as ZoneKind)}
          options={[
            { id: "Native", label: "Native" },
            { id: "Master", label: "Primary" },
            { id: "Slave", label: "Secondary" },
          ]}
        />
        <SelectField label="Account" value={account} onChange={setAccount} options={accountOptions} />
        <SelectField label="SOA-EDIT-API" value={soaEditApi} onChange={setSoaEditApi} options={SOA_EDIT_API_OPTIONS} />
      </FieldGrid>
      {kind === "Slave" && (
        <TextAreaField
          label="Primary servers"
          description="One IP address per line, with an optional port."
          value={masters}
          onChange={setMasters}
          errorMessage={mastersError}
          mono
        />
      )}
      <Switch
        isSelected={rectify}
        onChange={setRectify}
        description="Rectify the zone after each change made through the API."
      >
        Rectify on change
      </Switch>
      <div>
        <Button type="submit" variant="primary" isPending={update.isPending}>
          Save settings
        </Button>
      </div>
    </Form>
  );
}

const ACTION_LABELS: Record<ZoneAction, string> = {
  notify: "Notify sent",
  rectify: "Zone rectified",
  "axfr-retrieve": "Transfer requested",
};

/** Zone settings and maintenance actions. */
export function SettingsTab({ zone }: { zone: Zone }) {
  const me = useMe();
  const navigate = useNavigate();
  const action = useZoneAction(zone.id);
  const exportZone = useZoneExport(zone.id);
  const remove = useDeleteZone(zone.id);
  const [confirmDelete, setConfirmDelete] = useState(false);

  return (
    <div className="flex flex-col gap-4">
      <Card title="Settings">
        <ZoneForm zone={zone} />
      </Card>
      <Card title="Actions">
        <div className="flex flex-col gap-3">
          <div className="flex flex-wrap gap-2">
            {zone.kind !== "Native" && (
              <Button onPress={() => action.mutate("notify")} isDisabled={action.isPending}>
                <Send aria-hidden />
                Send notify
              </Button>
            )}
            <Button onPress={() => action.mutate("rectify")} isDisabled={action.isPending}>
              <Wrench aria-hidden />
              Rectify
            </Button>
            {zone.kind === "Slave" && (
              <Button onPress={() => action.mutate("axfr-retrieve")} isDisabled={action.isPending}>
                <RefreshCw aria-hidden />
                Retrieve from primary
              </Button>
            )}
            <Button
              isPending={exportZone.isPending}
              onPress={() =>
                exportZone.mutate(undefined, {
                  onSuccess: (data) => downloadText(`${zone.name.replace(/\.$/, "")}.zone`, data.zone),
                })
              }
            >
              <Download aria-hidden />
              Export zone file
            </Button>
          </div>
          {action.isSuccess && (
            <Alert tone="success" title={ACTION_LABELS[action.variables]}>
              {action.data.result}
            </Alert>
          )}
          {action.isError && <Alert tone="danger">{errorMessage(action.error)}</Alert>}
          {exportZone.isError && <Alert tone="danger">{errorMessage(exportZone.error)}</Alert>}
        </div>
      </Card>
      {can(me, "delete_zones") && (
        <Card title="Delete zone">
          <div className="flex flex-wrap items-center justify-between gap-3">
            <p className="text-sm text-fg-muted">Deleting a zone removes it and all its records from PowerDNS.</p>
            <Button variant="danger" onPress={() => setConfirmDelete(true)}>
              Delete zone
            </Button>
          </div>
        </Card>
      )}
      <ConfirmDialog
        title="Delete zone"
        isOpen={confirmDelete}
        onOpenChange={setConfirmDelete}
        confirmLabel="Delete zone"
        requireText={zone.name}
        isPending={remove.isPending}
        error={remove.error}
        onConfirm={() =>
          remove.mutate(undefined, {
            onSuccess: () => {
              setConfirmDelete(false);
              void navigate({ to: "/zones" });
            },
          })
        }
      >
        <p>
          This permanently deletes <span className="font-mono">{zone.name}</span> and all of its records. It cannot be
          undone.
        </p>
      </ConfirmDialog>
    </div>
  );
}
