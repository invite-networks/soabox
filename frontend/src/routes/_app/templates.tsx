import { zodResolver } from "@hookform/resolvers/zod";
import { createFileRoute } from "@tanstack/react-router";
import { Pencil, Plus, Trash2 } from "lucide-react";
import { useEffect, useState } from "react";
import { Controller, useFieldArray, useForm } from "react-hook-form";
import { z } from "zod";
import { errorMessage } from "../../api/client";
import { useDeleteTemplate, useSaveTemplate, useTemplates } from "../../api/hooks/templates";
import type { Template } from "../../api/types";
import { ConfirmDialog } from "../../components/confirm-dialog";
import {
  Alert,
  Button,
  Card,
  Dialog,
  DialogFooter,
  EmptyState,
  ErrorState,
  FieldGrid,
  Form,
  LoadingState,
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
} from "../../components/ui";
import { RecordValueFields } from "../../components/zone/record-value-fields";
import { KNOWN_RECORD_TYPES } from "../../lib/dns";
import { useMe } from "../../lib/me-context";
import { can } from "../../lib/permissions";
import { emptyRow, type RowValue, rowContent, rowErrors, rowFromContent } from "../../lib/record-rows";

export const Route = createFileRoute("/_app/templates")({
  component: TemplatesPage,
});

const TYPES = [...new Set([...KNOWN_RECORD_TYPES, "SOA"])].sort();

const schema = z
  .object({
    name: z.string().trim().min(1, "Enter a name."),
    description: z.string(),
    records: z.array(
      z.object({
        name: z
          .string()
          .trim()
          .regex(/^(@|[A-Za-z0-9_*-]+(\.[A-Za-z0-9_-]+)*)$/, "Use @ or a relative name such as www."),
        type: z.string(),
        ttl: z.string().regex(/^\d+$/, "Enter a whole number of seconds."),
        values: z.array(z.string()),
        initial: z.array(z.string()).nullable(),
        original: z.string().nullable(),
        raw: z.boolean(),
      }),
    ),
  })
  .superRefine((values, ctx) => {
    values.records.forEach((r, i) => {
      rowErrors(r.type, r as RowValue, null).forEach((message, j) => {
        if (message) {
          ctx.addIssue({ code: "custom", path: ["records", i, "values", j], message });
        }
      });
    });
  });
type Values = z.infer<typeof schema>;

function toValues(t: Template | null): Values {
  return {
    name: t?.name ?? "",
    description: t?.description ?? "",
    records: (t?.records ?? []).map((r) => ({
      name: r.name,
      type: r.type,
      ttl: String(r.ttl),
      ...rowFromContent(r.type, r.content, null),
    })),
  };
}

function TemplateDialog({
  editing,
  isOpen,
  onOpenChange,
}: {
  editing: Template | null;
  isOpen: boolean;
  onOpenChange: (o: boolean) => void;
}) {
  const save = useSaveTemplate();
  const form = useForm<Values>({ resolver: zodResolver(schema), defaultValues: toValues(editing) });
  const records = useFieldArray({ control: form.control, name: "records" });

  useEffect(() => {
    if (isOpen) {
      form.reset(toValues(editing));
      save.reset();
    }
  }, [isOpen, editing, form, save.reset]);

  const submit = form.handleSubmit((values) => {
    save.mutate(
      {
        id: editing?.id ?? null,
        body: {
          name: values.name,
          description: values.description,
          records: values.records.map((r) => ({
            name: r.name,
            type: r.type,
            ttl: Number(r.ttl),
            content: rowContent(r.type, r as RowValue, null),
          })),
        },
      },
      { onSuccess: () => onOpenChange(false) },
    );
  });

  return (
    <Dialog
      title={editing ? `Edit ${editing.name}` : "Create template"}
      isOpen={isOpen}
      onOpenChange={onOpenChange}
      size="xl"
    >
      <Form label="Template" onSubmit={submit}>
        {save.isError && <Alert tone="danger">{errorMessage(save.error)}</Alert>}
        <FieldGrid>
          <Controller
            control={form.control}
            name="name"
            render={({ field, fieldState }) => (
              <TextField
                label="Name"
                value={field.value}
                onChange={field.onChange}
                onBlur={field.onBlur}
                inputRef={field.ref}
                errorMessage={fieldState.error?.message}
              />
            )}
          />
          <Controller
            control={form.control}
            name="description"
            render={({ field }) => <TextField label="Description" value={field.value} onChange={field.onChange} />}
          />
        </FieldGrid>
        <div className="flex flex-col gap-3">
          <div>
            <h3 className="text-sm font-medium text-fg">Records</h3>
            <p className="text-xs text-fg-muted">
              Names and host values are relative to the new zone unless they end with a dot. Use @ for the zone itself.
            </p>
          </div>
          {records.fields.length === 0 && <p className="text-sm text-fg-muted">No records yet.</p>}
          {records.fields.map((item, index) => {
            const rowType = form.watch(`records.${index}.type`);
            const errors = form.formState.errors.records?.[index];
            return (
              <fieldset key={item.id} className="relative flex flex-col gap-3 rounded-lg border border-border p-3">
                <legend className="float-left w-full pt-1 text-xs font-semibold tracking-wider text-fg-muted uppercase">
                  Record {index + 1}
                </legend>
                <div className="absolute top-2 right-2">
                  <Button
                    variant="ghost"
                    size="icon"
                    aria-label={`Remove record ${index + 1}`}
                    onPress={() => records.remove(index)}
                  >
                    <Trash2 aria-hidden />
                  </Button>
                </div>
                <div className="grid grid-cols-1 gap-2 sm:grid-cols-[minmax(0,1fr)_10rem_8rem]">
                  <Controller
                    control={form.control}
                    name={`records.${index}.name`}
                    render={({ field, fieldState }) => (
                      <TextField
                        label="Name"
                        placeholder="@"
                        value={field.value}
                        onChange={field.onChange}
                        inputRef={field.ref}
                        errorMessage={fieldState.error?.message}
                        mono
                        autoComplete="off"
                      />
                    )}
                  />
                  <Controller
                    control={form.control}
                    name={`records.${index}.type`}
                    render={({ field }) => (
                      <SelectField
                        label="Type"
                        value={field.value}
                        onChange={(value) => {
                          field.onChange(value);
                          form.setValue(`records.${index}`, {
                            ...form.getValues(`records.${index}`),
                            type: value,
                            ...emptyRow(value),
                          });
                        }}
                        options={TYPES.map((t) => ({ id: t, label: t }))}
                      />
                    )}
                  />
                  <Controller
                    control={form.control}
                    name={`records.${index}.ttl`}
                    render={({ field, fieldState }) => (
                      <TextField
                        label="TTL (seconds)"
                        inputMode="numeric"
                        value={field.value}
                        onChange={(value) => field.onChange(value.replace(/\D/g, ""))}
                        inputRef={field.ref}
                        errorMessage={fieldState.error?.message}
                        mono
                      />
                    )}
                  />
                </div>
                <Controller
                  control={form.control}
                  name={`records.${index}.values`}
                  render={({ field }) => (
                    <RecordValueFields
                      type={rowType}
                      values={field.value}
                      onChange={field.onChange}
                      errors={(errors?.values as { message?: string }[] | undefined)?.map((e) => e?.message)}
                      zone={null}
                      index={0}
                      raw={form.getValues(`records.${index}.raw`)}
                    />
                  )}
                />
              </fieldset>
            );
          })}
          <div>
            <Button size="sm" onPress={() => records.append({ name: "@", type: "A", ttl: "3600", ...emptyRow("A") })}>
              <Plus aria-hidden />
              Add record
            </Button>
          </div>
        </div>
        <DialogFooter>
          <Button onPress={() => onOpenChange(false)}>Cancel</Button>
          <Button type="submit" variant="primary" isPending={save.isPending}>
            Save template
          </Button>
        </DialogFooter>
      </Form>
    </Dialog>
  );
}

function TemplatesPage() {
  const me = useMe();
  const canManage = can(me, "manage_templates");
  const templates = useTemplates();
  const remove = useDeleteTemplate();
  const [dialog, setDialog] = useState<{ open: boolean; editing: Template | null }>({ open: false, editing: null });
  const [deleting, setDeleting] = useState<Template | null>(null);

  return (
    <>
      <PageHeader
        title="Templates"
        description="Records added to a new zone when it is created from a template."
        actions={
          canManage && (
            <Button variant="primary" onPress={() => setDialog({ open: true, editing: null })}>
              <Plus aria-hidden />
              Create template
            </Button>
          )
        }
      />
      <Card flush>
        {templates.isPending ? (
          <LoadingState />
        ) : templates.isError ? (
          <ErrorState error={templates.error} onRetry={() => void templates.refetch()} />
        ) : templates.data.length === 0 ? (
          <EmptyState title="No templates yet" />
        ) : (
          <Table label="Templates">
            <THead>
              <tr>
                <Th>Name</Th>
                <Th>Description</Th>
                <Th>Records</Th>
                {canManage && <Th hidden>Actions</Th>}
              </tr>
            </THead>
            <TBody>
              {templates.data.map((t) => (
                <Tr key={t.id}>
                  <RowHeader>{t.name}</RowHeader>
                  <Td wrap>{t.description}</Td>
                  <Td mono wrap>
                    {t.records.map((r) => `${r.name} ${r.type} ${r.content}`).join(", ") || "None"}
                  </Td>
                  {canManage && (
                    <Td align="right">
                      <RowActions>
                        <Button
                          variant="ghost"
                          size="icon"
                          aria-label={`Edit ${t.name}`}
                          onPress={() => setDialog({ open: true, editing: t })}
                        >
                          <Pencil aria-hidden />
                        </Button>
                        <Button
                          variant="ghost"
                          size="icon"
                          aria-label={`Delete ${t.name}`}
                          onPress={() => setDeleting(t)}
                        >
                          <Trash2 aria-hidden />
                        </Button>
                      </RowActions>
                    </Td>
                  )}
                </Tr>
              ))}
            </TBody>
          </Table>
        )}
      </Card>
      <TemplateDialog
        editing={dialog.editing}
        isOpen={dialog.open}
        onOpenChange={(open) => setDialog((d) => ({ ...d, open }))}
      />
      <ConfirmDialog
        title="Delete template"
        isOpen={deleting !== null}
        onOpenChange={(o) => !o && setDeleting(null)}
        confirmLabel="Delete template"
        isPending={remove.isPending}
        error={remove.error}
        onConfirm={() => deleting && remove.mutate(deleting.id, { onSuccess: () => setDeleting(null) })}
      >
        <p>Delete the template {deleting?.name}? Zones already created from it are not changed.</p>
      </ConfirmDialog>
    </>
  );
}
