import { Check, ChevronLeft, ChevronRight, Copy, Search, X } from "lucide-react";
import { type ReactNode, useEffect, useState } from "react";
import {
  Input,
  Label,
  ListBox,
  ListBoxItem,
  Popover,
  Button as RACButton,
  ComboBox as RACComboBox,
  Form as RACForm,
  SearchField as RACSearchField,
  Text,
} from "react-aria-components";
import { Button } from "./Button";
import { fieldInput, fieldLabel, focusRing, listItem, popover } from "./styles";

/** A form wrapper. Validation messages come from the field components. */
export function Form({ onSubmit, children, label }: { onSubmit: () => void; children: ReactNode; label?: string }) {
  return (
    <RACForm
      {...(label ? { "aria-label": label } : {})}
      validationBehavior="aria"
      onSubmit={(event) => {
        event.preventDefault();
        onSubmit();
      }}
      className="flex flex-col gap-3"
    >
      {children}
    </RACForm>
  );
}

export interface CopyButtonProps {
  text: string;
  label?: string;
}

/** Copies a value to the clipboard and confirms it. */
export function CopyButton({ text, label = "Copy" }: CopyButtonProps) {
  const [state, setState] = useState<"idle" | "copied" | "failed">("idle");
  useEffect(() => {
    if (state === "idle") {
      return;
    }
    const timer = window.setTimeout(() => setState("idle"), 2000);
    return () => window.clearTimeout(timer);
  }, [state]);
  return (
    <span className="inline-flex items-center gap-1.5">
      <Button
        size="sm"
        onPress={async () => {
          try {
            await navigator.clipboard.writeText(text);
            setState("copied");
          } catch {
            setState("failed");
          }
        }}
      >
        {state === "copied" ? <Check aria-hidden /> : <Copy aria-hidden />}
        {label}
      </Button>
      <span aria-live="polite" className="text-xs text-fg-muted">
        {state === "copied" ? "Copied" : state === "failed" ? "Copy failed. Select the text and copy it." : ""}
      </span>
    </span>
  );
}

export interface SearchFieldProps {
  label: string;
  value: string;
  onChange: (value: string) => void;
  onSubmit?: (value: string) => void;
  placeholder?: string;
  hideLabel?: boolean;
}

/** A search input with a clear button. */
export function SearchField({ label, value, onChange, onSubmit, placeholder, hideLabel }: SearchFieldProps) {
  return (
    <RACSearchField
      value={value}
      onChange={onChange}
      {...(onSubmit ? { onSubmit } : {})}
      className="group flex flex-col gap-1"
    >
      <Label className={hideLabel ? "sr-only" : fieldLabel}>{label}</Label>
      <div className="relative">
        <Search
          aria-hidden
          className="pointer-events-none absolute top-1/2 left-2.5 size-4 -translate-y-1/2 text-fg-muted"
        />
        <Input
          {...(placeholder ? { placeholder } : {})}
          className={`${fieldInput} pr-8 pl-8 [&::-webkit-search-cancel-button]:hidden`}
        />
        <RACButton
          className={`absolute top-1/2 right-1.5 -translate-y-1/2 rounded p-0.5 text-fg-muted group-empty:hidden hovered:text-fg ${focusRing}`}
        >
          <X aria-hidden className="size-4" />
        </RACButton>
      </div>
    </RACSearchField>
  );
}

export interface ComboItem {
  id: string;
  label: string;
}

export interface QuickSearchProps {
  label: string;
  placeholder?: string;
  inputValue: string;
  onInputChange: (value: string) => void;
  items: readonly ComboItem[];
  onSelect: (id: string) => void;
  emptyText: string;
}

/** A search box with suggestions. Choosing a suggestion calls `onSelect`. */
export function QuickSearch(props: QuickSearchProps) {
  return (
    <RACComboBox
      aria-label={props.label}
      inputValue={props.inputValue}
      onInputChange={props.onInputChange}
      items={props.items}
      value={null}
      onChange={(key) => {
        if (key !== null) {
          props.onSelect(String(key));
        }
      }}
      menuTrigger="input"
      allowsEmptyCollection
      className="relative w-full max-w-sm"
    >
      <Search
        aria-hidden
        className="pointer-events-none absolute top-1/2 left-2.5 size-4 -translate-y-1/2 text-fg-muted"
      />
      <Input {...(props.placeholder ? { placeholder: props.placeholder } : {})} className={`${fieldInput} pl-8`} />
      <Popover className={popover} placement="bottom start">
        <ListBox
          className="outline-none"
          renderEmptyState={() => <Text className="block px-2 py-1.5 text-sm text-fg-muted">{props.emptyText}</Text>}
        >
          {(item: ComboItem) => (
            <ListBoxItem id={item.id} textValue={item.label} className={`${listItem} font-mono text-[13px]`}>
              {item.label}
            </ListBoxItem>
          )}
        </ListBox>
      </Popover>
    </RACComboBox>
  );
}

export interface PaginationProps {
  page: number;
  pageCount: number;
  total: number;
  onPageChange: (page: number) => void;
  noun?: string;
}

/** Previous and next page controls with a summary. */
export function Pagination({ page, pageCount, total, onPageChange, noun = "items" }: PaginationProps) {
  return (
    <nav
      aria-label="Pagination"
      className="flex flex-wrap items-center justify-between gap-2 px-3 py-2 text-sm text-fg-muted"
    >
      <span>
        {total} {noun}
        {pageCount > 1 ? `, page ${page} of ${pageCount}` : ""}
      </span>
      {pageCount > 1 && (
        <div className="flex gap-1">
          <Button size="sm" isDisabled={page <= 1} onPress={() => onPageChange(page - 1)}>
            <ChevronLeft aria-hidden />
            Previous
          </Button>
          <Button size="sm" isDisabled={page >= pageCount} onPress={() => onPageChange(page + 1)}>
            Next
            <ChevronRight aria-hidden />
          </Button>
        </div>
      )}
    </nav>
  );
}
