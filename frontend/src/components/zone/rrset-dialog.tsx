import { zodResolver } from "@hookform/resolvers/zod";
import { Plus, Trash2 } from "lucide-react";
import { useEffect } from "react";
import { Controller, useFieldArray, useForm } from "react-hook-form";
import { z } from "zod";
import type { RrSet } from "../../api/types";
import {
  EDIT_ONLY_TYPES,
  isInZone,
  normalizeContent,
  toAbsoluteName,
  toRelativeName,
  validateContent,
} from "../../lib/dns";
import { Alert, Button, Checkbox, Dialog, DialogFooter, FieldGrid, Form, SelectField, TextField } from "../ui";

const DEFAULT_TTL = 3600;

const PLACEHOLDERS: Record<string, string> = {
  A: "192.0.2.10",
  AAAA: "2001:db8::10",
  CNAME: "target.example.com.",
  MX: "10 mail.example.com.",
  NS: "ns1.example.com.",
  PTR: "host.example.com.",
  SRV: "10 5 5060 sip.example.com.",
  TXT: "v=spf1 -all",
  CAA: '0 issue "ca.example.net"',
};

const formSchema = z.object({
  name: z.string(),
  type: z.string().min(1, "Choose a type."),
  ttl: z.string().regex(/^\d+$/, "Enter a whole number of seconds."),
  records: z.array(z.object({ content: z.string(), disabled: z.boolean() })).min(1),
});
type FormValues = z.infer<typeof formSchema>;

export interface RrSetDialogProps {
  zoneName: string;
  isOpen: boolean;
  onOpenChange: (open: boolean) => void;
  /** The RRset being edited, or null to add. */
  editing: RrSet | null;
  allowedTypes: readonly string[];
  /** The current working RRsets, used for conflict checks. */
  existing: readonly RrSet[];
  onSave: (rrset: RrSet) => void;
}

function initialValues(editing: RrSet | null, zoneName: string, defaultType: string): FormValues {
  if (editing) {
    return {
      name: toRelativeName(editing.name, zoneName),
      type: editing.type,
      ttl: String(editing.ttl),
      records: editing.records.map((r) => ({ content: r.content, disabled: r.disabled })),
    };
  }
  return { name: "", type: defaultType, ttl: String(DEFAULT_TTL), records: [{ content: "", disabled: false }] };
}

/** Add or edit one RRset. The result is collected as a pending change. */
export function RrSetDialog(props: RrSetDialogProps) {
  const { zoneName, editing, allowedTypes, existing } = props;
  const isEdit = editing !== null;
  const defaultType = allowedTypes.includes("A") ? "A" : (allowedTypes[0] ?? "A");

  const schema = formSchema.superRefine((values, ctx) => {
    const absolute = toAbsoluteName(values.name, zoneName);
    if (!isEdit) {
      if (!isInZone(absolute, zoneName)) {
        ctx.addIssue({ code: "custom", path: ["name"], message: `The name must be inside ${zoneName}` });
      }
      const others = existing.filter((r) => r.name.toLowerCase() === absolute.toLowerCase());
      if (values.type === "CNAME" && others.some((r) => r.type !== "CNAME")) {
        ctx.addIssue({ code: "custom", path: ["name"], message: "A CNAME cannot share a name with other records." });
      }
      if (values.type !== "CNAME" && others.some((r) => r.type === "CNAME")) {
        ctx.addIssue({ code: "custom", path: ["name"], message: "This name already has a CNAME record." });
      }
    }
    values.records.forEach((record, index) => {
      const message = validateContent(values.type, record.content);
      if (message) {
        ctx.addIssue({ code: "custom", path: ["records", index, "content"], message });
      }
    });
    const normalized = values.records.map((r) => normalizeContent(values.type, r.content));
    if (new Set(normalized).size !== normalized.length) {
      ctx.addIssue({ code: "custom", path: ["records", 0, "content"], message: "Each value must be unique." });
    }
  });

  const form = useForm<FormValues>({
    resolver: zodResolver(schema),
    defaultValues: initialValues(editing, zoneName, defaultType),
  });
  const records = useFieldArray({ control: form.control, name: "records" });
  const type = form.watch("type");
  const singleRecord = EDIT_ONLY_TYPES.has(type) || type === "CNAME";

  useEffect(() => {
    if (props.isOpen) {
      form.reset(initialValues(editing, zoneName, defaultType));
    }
  }, [props.isOpen, editing, zoneName, defaultType, form]);

  const submit = form.handleSubmit((values) => {
    props.onSave({
      name: toAbsoluteName(values.name, zoneName),
      type: values.type,
      ttl: Number(values.ttl),
      records: values.records.map((r) => ({ content: normalizeContent(values.type, r.content), disabled: r.disabled })),
      comments: editing?.comments ?? [],
    });
    props.onOpenChange(false);
  });

  const typeOptions = (isEdit ? [editing.type] : allowedTypes.filter((t) => !EDIT_ONLY_TYPES.has(t))).map((t) => ({
    id: t,
    label: t,
  }));
  const rootError = form.formState.errors.records?.root?.message ?? form.formState.errors.records?.message;

  return (
    <Dialog
      title={isEdit ? `Edit ${editing.type} records` : "Add records"}
      isOpen={props.isOpen}
      onOpenChange={props.onOpenChange}
      size="lg"
    >
      <Form onSubmit={submit} label={isEdit ? "Edit records" : "Add records"}>
        <FieldGrid columns={3}>
          <Controller
            control={form.control}
            name="name"
            render={({ field, fieldState }) => (
              <TextField
                label="Name"
                description={`Relative to ${zoneName} Use @ for the zone itself.`}
                placeholder="@"
                value={field.value}
                onChange={field.onChange}
                onBlur={field.onBlur}
                inputRef={field.ref}
                errorMessage={fieldState.error?.message}
                isReadOnly={isEdit}
                mono
                autoComplete="off"
              />
            )}
          />
          <Controller
            control={form.control}
            name="type"
            render={({ field, fieldState }) => (
              <SelectField
                label="Type"
                value={field.value}
                onChange={field.onChange}
                options={typeOptions}
                isDisabled={isEdit}
                errorMessage={fieldState.error?.message}
              />
            )}
          />
          <Controller
            control={form.control}
            name="ttl"
            render={({ field, fieldState }) => (
              <TextField
                label="TTL (seconds)"
                inputMode="numeric"
                value={field.value}
                onChange={field.onChange}
                onBlur={field.onBlur}
                inputRef={field.ref}
                errorMessage={fieldState.error?.message}
                mono
              />
            )}
          />
        </FieldGrid>
        <fieldset className="flex flex-col gap-2">
          <legend className="mb-1 text-sm font-medium text-fg">Values</legend>
          {records.fields.map((item, index) => (
            <div key={item.id} className="flex items-start gap-2">
              <div className="flex-1">
                <Controller
                  control={form.control}
                  name={`records.${index}.content`}
                  render={({ field, fieldState }) => (
                    <TextField
                      label={`Value ${index + 1}`}
                      hideLabel
                      placeholder={PLACEHOLDERS[type] ?? ""}
                      value={field.value}
                      onChange={field.onChange}
                      onBlur={field.onBlur}
                      inputRef={field.ref}
                      errorMessage={fieldState.error?.message}
                      mono
                      autoComplete="off"
                    />
                  )}
                />
              </div>
              <div className="flex h-8 items-center gap-2">
                <Controller
                  control={form.control}
                  name={`records.${index}.disabled`}
                  render={({ field }) => (
                    <Checkbox isSelected={field.value} onChange={field.onChange}>
                      Disabled
                    </Checkbox>
                  )}
                />
                <Button
                  variant="ghost"
                  size="icon"
                  aria-label={`Remove value ${index + 1}`}
                  isDisabled={records.fields.length <= 1}
                  onPress={() => records.remove(index)}
                >
                  <Trash2 aria-hidden />
                </Button>
              </div>
            </div>
          ))}
          {rootError && <Alert tone="danger">{rootError}</Alert>}
          {!singleRecord && (
            <div>
              <Button size="sm" onPress={() => records.append({ content: "", disabled: false })}>
                <Plus aria-hidden />
                Add value
              </Button>
            </div>
          )}
        </fieldset>
        <DialogFooter>
          <Button onPress={() => props.onOpenChange(false)}>Cancel</Button>
          <Button type="submit" variant="primary">
            {isEdit ? "Save to pending changes" : "Add to pending changes"}
          </Button>
        </DialogFooter>
      </Form>
    </Dialog>
  );
}
