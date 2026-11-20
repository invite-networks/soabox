/**
 * Editor state for one record value, shared by the zone record dialog and templates.
 */
import { normalizeContent, validateContent } from "./dns";
import { composeFields, emptyFields, fieldsFor, parseFields, RAW_FIELDS, validateField } from "./record-fields";

export interface RowValue {
  /** One entry per field. */
  values: string[];
  /** The values as first loaded, used to keep untouched content byte for byte. */
  initial: string[] | null;
  /** The stored content this row was loaded from. */
  original: string | null;
  /** True when the content does not fit the typed fields and is edited as one value. */
  raw: boolean;
}

export function emptyRow(type: string): RowValue {
  return { values: emptyFields(type), initial: null, original: null, raw: false };
}

/** Loads stored content into editor fields, falling back to one free-form field. */
export function rowFromContent(type: string, content: string, zone: string | null): RowValue {
  const parsed = parseFields(type, content, zone);
  if (parsed) {
    return { values: parsed, initial: [...parsed], original: content, raw: false };
  }
  return { values: [content], initial: [content], original: content, raw: true };
}

function unchanged(row: RowValue): boolean {
  return (
    row.original !== null &&
    row.initial !== null &&
    row.initial.length === row.values.length &&
    row.initial.every((v, i) => v === row.values[i])
  );
}

/** The content to store for a row. Untouched values are kept exactly as they were. */
export function rowContent(type: string, row: RowValue, zone: string | null): string {
  if (unchanged(row) && row.original !== null) {
    return row.original;
  }
  if (row.raw) {
    return normalizeContent(type, row.values[0] ?? "");
  }
  return composeFields(type, row.values, zone);
}

/** Field errors for a row, in field order. Untouched values are not checked again. */
export function rowErrors(type: string, row: RowValue, zone: string | null): (string | null)[] {
  if (unchanged(row)) {
    return [];
  }
  if (row.raw) {
    return [validateContent(type, row.values[0] ?? "")];
  }
  const defs = row.raw ? RAW_FIELDS : fieldsFor(type);
  return defs.map((def, i) => validateField(def, row.values[i] ?? "", zone));
}
