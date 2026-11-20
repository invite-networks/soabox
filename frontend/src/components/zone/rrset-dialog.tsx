import { zodResolver } from "@hookform/resolvers/zod";
import { Plus, Trash2 } from "lucide-react";
import { useEffect, useRef } from "react";
import { Controller, useFieldArray, useForm } from "react-hook-form";
import { z } from "zod";
import type { RrSet } from "../../api/types";
import { EDIT_ONLY_TYPES, isInZone, toAbsoluteName, toRelativeName } from "../../lib/dns";
import { emptyRow, type RowValue, rowContent, rowErrors, rowFromContent } from "../../lib/record-rows";
import { Alert, Button, Checkbox, Dialog, DialogFooter, FieldGrid, Form, SelectField, TextField } from "../ui";
import { RecordValueFields } from "./record-value-fields";

const DEFAULT_TTL = 3600;

const formSchema = z.object({
  name: z.string(),
  type: z.string().min(1, "Choose a type."),
  ttl: z.string().regex(/^\d+$/, "Enter a whole number of seconds."),
  records: z
    .array(
      z.object({
        values: z.array(z.string()),
        initial: z.array(z.string()).nullable(),
        original: z.string().nullable(),
        raw: z.boolean(),
        disabled: z.boolean(),
      }),
    )
    .min(1),
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
      records: editing.records.map((r) => ({
        ...rowFromContent(editing.type, r.content, zoneName),
        disabled: r.disabled,
      })),
    };
  }
  return {
    name: "",
    type: defaultType,
    ttl: String(DEFAULT_TTL),
    records: [{ ...emptyRow(defaultType), disabled: false }],
  };
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
      rowErrors(values.type, record as RowValue, zoneName).forEach((message, j) => {
        if (message) {
          ctx.addIssue({ code: "custom", path: ["records", index, "values", j], message });
        }
      });
    });
    const contents = values.records.map((r) => rowContent(values.type, r as RowValue, zoneName));
    if (new Set(contents).size !== contents.length) {
      ctx.addIssue({ code: "custom", path: ["records", "root"], message: "Each value must be unique." });
    }
  });

  const form = useForm<FormValues>({
    resolver: zodResolver(schema),
    defaultValues: initialValues(editing, zoneName, defaultType),
  });
  const records = useFieldArray({ control: form.control, name: "records" });
  const type = form.watch("type");
  const singleRecord = EDIT_ONLY_TYPES.has(type) || type === "CNAME";
  const valuesRef = useRef<HTMLFieldSetElement>(null);

  useEffect(() => {
    if (props.isOpen) {
      form.reset(initialValues(editing, zoneName, defaultType));
    }
  }, [props.isOpen, editing, zoneName, defaultType, form]);

  const submit = form.handleSubmit(
    (values) => {
      props.onSave({
        name: toAbsoluteName(values.name, zoneName),
        type: values.type,
        ttl: Number(values.ttl),
        records: values.records.map((r) => ({
          content: rowContent(values.type, r as RowValue, zoneName),
          disabled: r.disabled,
        })),
        comments: editing?.comments ?? [],
      });
      props.onOpenChange(false);
    },
    (errors) => {
      if (!errors.name && !errors.ttl) {
        // Value fields are controlled groups, so move focus to the first invalid input ourselves.
        requestAnimationFrame(() => valuesRef.current?.querySelector<HTMLElement>('[aria-invalid="true"]')?.focus());
      }
    },
  );

  const typeOptions = (isEdit ? [editing.type] : allowedTypes.filter((t) => !EDIT_ONLY_TYPES.has(t))).map((t) => ({
    id: t,
    label: t,
  }));
  const recordErrors = form.formState.errors.records;
  const rootError = recordErrors?.root?.message ?? recordErrors?.message;

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
                onChange={(value) => {
                  field.onChange(value);
                  records.replace([{ ...emptyRow(value), disabled: false }]);
                }}
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
                onChange={(value) => field.onChange(value.replace(/\D/g, ""))}
                onBlur={field.onBlur}
                inputRef={field.ref}
                errorMessage={fieldState.error?.message}
                mono
              />
            )}
          />
        </FieldGrid>
        <fieldset ref={valuesRef} className="flex flex-col gap-3">
          <legend className="mb-1 text-sm font-medium text-fg">{records.fields.length > 1 ? "Values" : "Value"}</legend>
          {records.fields.map((item, index) => (
            <div key={item.id} className="flex items-start gap-2">
              <Controller
                control={form.control}
                name={`records.${index}.values`}
                render={({ field }) => (
                  <RecordValueFields
                    type={type}
                    values={field.value}
                    onChange={field.onChange}
                    errors={(recordErrors?.[index]?.values as { message?: string }[] | undefined)?.map(
                      (e) => e?.message,
                    )}
                    zone={zoneName}
                    index={index}
                    raw={form.getValues(`records.${index}.raw`)}
                  />
                )}
              />
              <div className={`flex h-8 shrink-0 items-center gap-2 ${index === 0 ? "mt-6" : ""}`}>
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
              <Button size="sm" onPress={() => records.append({ ...emptyRow(type), disabled: false })}>
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
