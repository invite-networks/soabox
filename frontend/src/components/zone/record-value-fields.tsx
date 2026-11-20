import {
  displayParts,
  type FieldDef,
  fieldsFor,
  getsZoneSuffix,
  inputModeFor,
  RAW_FIELDS,
  restrictInput,
} from "../../lib/record-fields";
import { SelectField, TextAreaField, TextField } from "../ui";

export interface RecordValueFieldsProps {
  type: string;
  values: readonly string[];
  onChange: (values: string[]) => void;
  errors?: readonly (string | undefined)[] | undefined;
  /** The zone relative names resolve to, or null in a template. */
  zone: string | null;
  /** Position of this value in its RRset. Labels are visible on the first value only. */
  index: number;
  /** Show one free-form field instead of the typed fields. */
  raw?: boolean;
}

function widthFor(def: FieldDef): string {
  if (def.wide) {
    return "min-w-0 flex-[1_1_16rem]";
  }
  if (def.kind === "select") {
    return "w-44 shrink-0";
  }
  return "w-32 shrink-0";
}

/** One input per part of a record value, restricted to what that part may contain. */
export function RecordValueFields({ type, values, onChange, errors, zone, index, raw }: RecordValueFieldsProps) {
  const defs = raw ? RAW_FIELDS : fieldsFor(type);
  const set = (i: number, value: string) => {
    const next = defs.map((_, j) => values[j] ?? "");
    next[i] = value;
    onChange(next);
  };

  return (
    <div className="flex min-w-0 flex-1 flex-wrap items-start gap-2">
      {defs.map((def, i) => {
        const label = index === 0 ? def.label : `${def.label} ${index + 1}`;
        const value = values[i] ?? "";
        const error = errors?.[i];
        const common = {
          label,
          hideLabel: index > 0,
          value,
          errorMessage: error,
        };
        let control: React.ReactNode;
        if (def.kind === "select") {
          const options = def.options ?? [];
          const all =
            value && !options.some((o) => o.id === value) ? [...options, { id: value, label: value }] : options;
          control = <SelectField {...common} onChange={(v) => set(i, v)} options={all} />;
        } else if (def.multiline) {
          control = (
            <TextAreaField
              {...common}
              onChange={(v) => set(i, restrictInput(def, v))}
              rows={2}
              mono
              {...(def.placeholder ? { placeholder: def.placeholder } : {})}
            />
          );
        } else {
          const suffix = getsZoneSuffix(def, value) ? `.${zone ?? "<zone>"}` : def.unit;
          control = (
            <TextField
              {...common}
              onChange={(v) => set(i, restrictInput(def, v))}
              inputMode={inputModeFor(def)}
              suffix={suffix}
              mono
              autoComplete="off"
              {...(def.placeholder ? { placeholder: def.placeholder } : {})}
            />
          );
        }
        return (
          <div key={def.key} className={widthFor(def)}>
            {control}
          </div>
        );
      })}
    </div>
  );
}

/** Shows a stored value with the zone suffix of in-zone names in a muted color. */
export function RecordContent({ type, content, zone }: { type: string; content: string; zone: string }) {
  return (
    <>
      {displayParts(type, content, zone).map((part, i) =>
        part.suffix ? (
          // biome-ignore lint/suspicious/noArrayIndexKey: parts are positional and never reorder.
          <span key={i} className="text-fg-muted">
            {part.text}
          </span>
        ) : (
          // biome-ignore lint/suspicious/noArrayIndexKey: parts are positional and never reorder.
          <span key={i}>{part.text}</span>
        ),
      )}
    </>
  );
}
