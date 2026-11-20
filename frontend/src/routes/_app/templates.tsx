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
import { DEFAULT_FORWARD_TYPES, normalizeContent, validateContent } from "../../lib/dns";
import { useMe } from "../../lib/me-context";
import { can } from "../../lib/permissions";

export const Route = createFileRoute("/_app/templates")({
  component: TemplatesPage,
});

const TYPES = [...new Set([...DEFAULT_FORWARD_TYPES, "PTR", "SOA"])].sort();

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
        ttl: z.string().regex(/^\d+$/, "Seconds"),
        content: z.string(),
      }),
    ),
  })
  .superRefine((values, ctx) => {
    values.records.forEach((r, i) => {
      const message = validateContent(r.type, r.content);
      if (message) {
        ctx.addIssue({ code: "custom", path: ["records", i, "content"], message });
      }
    });
  });
type Values = z.infer<typeof schema>;

function toValues(t: Template | null): Values {
  return {
    name: t?.name ?? "",
    description: t?.description ?? "",
    records: (t?.records ?? []).map((r) => ({ name: r.name, type: r.type, ttl: String(r.ttl), content: r.content })),
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
            content: normalizeContent(r.type, r.content),
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
        <fieldset className="flex flex-col gap-2">
          <legend className="mb-1 text-sm font-medium text-fg">Records</legend>
          <p className="text-xs text-fg-muted">Names are relative to the zone. Use @ for the zone itself.</p>
          {records.fields.map((item, index) => (
            <div key={item.id} className="grid grid-cols-[1fr_7rem_6rem_2fr_auto] items-start gap-2">
              <Controller
                control={form.control}
                name={`records.${index}.name`}
                render={({ field, fieldState }) => (
                  <TextField
                    label={`Record ${index + 1} name`}
                    hideLabel
                    placeholder="@"
                    value={field.value}
                    onChange={field.onChange}
                    inputRef={field.ref}
                    errorMessage={fieldState.error?.message}
                    mono
                  />
                )}
              />
              <Controller
                control={form.control}
                name={`records.${index}.type`}
                render={({ field }) => (
                  <SelectField
                    label={`Record ${index + 1} type`}
                    hideLabel
                    value={field.value}
                    onChange={field.onChange}
                    options={TYPES.map((t) => ({ id: t, label: t }))}
                  />
                )}
              />
              <Controller
                control={form.control}
                name={`records.${index}.ttl`}
                render={({ field, fieldState }) => (
                  <TextField
                    label={`Record ${index + 1} TTL`}
                    hideLabel
                    value={field.value}
                    onChange={field.onChange}
                    inputRef={field.ref}
                    errorMessage={fieldState.error?.message}
                    mono
                  />
                )}
              />
              <Controller
                control={form.control}
                name={`records.${index}.content`}
                render={({ field, fieldState }) => (
                  <TextField
                    label={`Record ${index + 1} content`}
                    hideLabel
                    value={field.value}
                    onChange={field.onChange}
                    inputRef={field.ref}
                    errorMessage={fieldState.error?.message}
                    mono
                  />
                )}
              />
              <Button
                variant="ghost"
                size="icon"
                aria-label={`Remove record ${index + 1}`}
                onPress={() => records.remove(index)}
              >
                <Trash2 aria-hidden />
              </Button>
            </div>
          ))}
          <div>
            <Button size="sm" onPress={() => records.append({ name: "@", type: "A", ttl: "3600", content: "" })}>
              <Plus aria-hidden />
              Add record
            </Button>
          </div>
        </fieldset>
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
