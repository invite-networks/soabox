import { zodResolver } from "@hookform/resolvers/zod";
import { useNavigate } from "@tanstack/react-router";
import { Controller, useForm } from "react-hook-form";
import { z } from "zod";
import { errorMessage } from "../api/client";
import { useAccounts } from "../api/hooks/accounts";
import { useTemplates } from "../api/hooks/templates";
import { useCreateZone } from "../api/hooks/zones";
import type { CreateZoneRequest } from "../api/types";
import { ensureDot, isHostname, isIPv4, isIPv6 } from "../lib/dns";
import { splitList } from "../lib/format";
import { Alert, Button, Dialog, DialogFooter, FieldGrid, Form, SelectField, TextAreaField, TextField } from "./ui";

const NONE = "none";

export const SOA_EDIT_API_OPTIONS = [
  { id: "DEFAULT", label: "DEFAULT" },
  { id: "INCREASE", label: "INCREASE" },
  { id: "EPOCH", label: "EPOCH" },
  { id: "OFF", label: "OFF" },
] as const;

function isPrimaryAddress(value: string): boolean {
  const host = value.replace(/:\d+$/, "").replace(/^\[(.*)\]$/, "$1");
  return isIPv4(host) || isIPv6(value) || isIPv6(host);
}

const schema = z
  .object({
    name: z
      .string()
      .trim()
      .refine((v) => isHostname(v), "Enter a valid zone name, for example example.com."),
    kind: z.enum(["Native", "Master", "Slave"]),
    masters: z.string(),
    nameservers: z.string(),
    template: z.string(),
    account: z.string(),
    soaEditApi: z.string(),
  })
  .superRefine((value, ctx) => {
    const masters = splitList(value.masters);
    if (value.kind === "Slave" && masters.length === 0) {
      ctx.addIssue({ code: "custom", path: ["masters"], message: "Enter at least one primary server address." });
    }
    for (const m of masters) {
      if (!isPrimaryAddress(m)) {
        ctx.addIssue({ code: "custom", path: ["masters"], message: `${m} is not an IP address.` });
        break;
      }
    }
    for (const ns of splitList(value.nameservers)) {
      if (!isHostname(ns)) {
        ctx.addIssue({ code: "custom", path: ["nameservers"], message: `${ns} is not a host name.` });
        break;
      }
    }
  });
type Values = z.infer<typeof schema>;

const defaults: Values = {
  name: "",
  kind: "Native",
  masters: "",
  nameservers: "",
  template: NONE,
  account: NONE,
  soaEditApi: NONE,
};

/** Builds the create request from form values. */
export function toCreateZoneRequest(values: Values): CreateZoneRequest {
  const body: CreateZoneRequest = {
    name: ensureDot(values.name.trim()),
    kind: values.kind,
    masters: values.kind === "Slave" ? splitList(values.masters) : [],
    nameservers: values.kind === "Slave" ? [] : splitList(values.nameservers).map(ensureDot),
  };
  if (values.template !== NONE) {
    body.template_id = Number(values.template);
  }
  if (values.account !== NONE) {
    body.account_id = Number(values.account);
  }
  if (values.soaEditApi !== NONE) {
    body.soa_edit_api = values.soaEditApi;
  }
  return body;
}

export function CreateZoneDialog({ isOpen, onOpenChange }: { isOpen: boolean; onOpenChange: (open: boolean) => void }) {
  const navigate = useNavigate();
  const create = useCreateZone();
  const templates = useTemplates();
  const accounts = useAccounts(isOpen);
  const form = useForm<Values>({ resolver: zodResolver(schema), defaultValues: defaults });
  const kind = form.watch("kind");

  const close = (open: boolean) => {
    if (!open) {
      form.reset(defaults);
      create.reset();
    }
    onOpenChange(open);
  };

  const submit = form.handleSubmit((values) => {
    create.mutate(toCreateZoneRequest(values), {
      onSuccess: (zone) => {
        close(false);
        void navigate({ to: "/zones/$zoneId", params: { zoneId: zone.id } });
      },
    });
  });

  return (
    <Dialog title="Create zone" isOpen={isOpen} onOpenChange={close} size="lg">
      <Form onSubmit={submit} label="Create zone">
        {create.isError && <Alert tone="danger">{errorMessage(create.error)}</Alert>}
        <FieldGrid>
          <Controller
            control={form.control}
            name="name"
            render={({ field, fieldState }) => (
              <TextField
                label="Zone name"
                placeholder="example.com"
                value={field.value}
                onChange={field.onChange}
                onBlur={field.onBlur}
                inputRef={field.ref}
                errorMessage={fieldState.error?.message}
                isRequired
                mono
                autoComplete="off"
              />
            )}
          />
          <Controller
            control={form.control}
            name="kind"
            render={({ field }) => (
              <SelectField
                label="Kind"
                value={field.value}
                onChange={field.onChange}
                options={[
                  { id: "Native", label: "Native" },
                  { id: "Master", label: "Primary" },
                  { id: "Slave", label: "Secondary" },
                ]}
              />
            )}
          />
        </FieldGrid>
        {kind === "Slave" ? (
          <Controller
            control={form.control}
            name="masters"
            render={({ field, fieldState }) => (
              <TextAreaField
                label="Primary servers"
                description="One IP address per line, with an optional port."
                value={field.value}
                onChange={field.onChange}
                onBlur={field.onBlur}
                inputRef={field.ref}
                errorMessage={fieldState.error?.message}
                mono
              />
            )}
          />
        ) : (
          <Controller
            control={form.control}
            name="nameservers"
            render={({ field, fieldState }) => (
              <TextAreaField
                label="Name servers"
                description="One host name per line. Leave empty when the template adds NS records."
                value={field.value}
                onChange={field.onChange}
                onBlur={field.onBlur}
                inputRef={field.ref}
                errorMessage={fieldState.error?.message}
                mono
              />
            )}
          />
        )}
        <FieldGrid columns={3}>
          <Controller
            control={form.control}
            name="template"
            render={({ field }) => (
              <SelectField
                label="Template"
                value={field.value}
                onChange={field.onChange}
                options={[
                  { id: NONE, label: "No template" },
                  ...(templates.data ?? []).map((t) => ({ id: String(t.id), label: t.name })),
                ]}
              />
            )}
          />
          <Controller
            control={form.control}
            name="account"
            render={({ field }) => (
              <SelectField
                label="Account"
                value={field.value}
                onChange={field.onChange}
                options={[
                  { id: NONE, label: "No account" },
                  ...(accounts.data ?? []).map((a) => ({ id: String(a.id), label: a.name })),
                ]}
              />
            )}
          />
          <Controller
            control={form.control}
            name="soaEditApi"
            render={({ field }) => (
              <SelectField
                label="SOA-EDIT-API"
                value={field.value}
                onChange={field.onChange}
                options={[{ id: NONE, label: "Server default" }, ...SOA_EDIT_API_OPTIONS]}
              />
            )}
          />
        </FieldGrid>
        <DialogFooter>
          <Button onPress={() => close(false)}>Cancel</Button>
          <Button type="submit" variant="primary" isPending={create.isPending}>
            Create zone
          </Button>
        </DialogFooter>
      </Form>
    </Dialog>
  );
}
