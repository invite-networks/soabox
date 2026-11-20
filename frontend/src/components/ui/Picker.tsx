import { Plus, X } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import {
  FieldError,
  Input,
  Label,
  ListBox,
  ListBoxItem,
  Popover,
  Button as RACButton,
  ComboBox as RACComboBox,
  Tag,
  TagGroup,
  TagList,
  Text,
} from "react-aria-components";
import type { ComboItem } from "./Misc";
import { fieldDescription, fieldError, fieldInput, fieldLabel, focusRing, listItem, popover } from "./styles";

export interface MultiPickerFieldProps {
  label: string;
  description?: string;
  placeholder?: string;
  /** The chosen values, shown as removable tags. */
  value: readonly string[];
  onChange: (value: string[]) => void;
  inputValue: string;
  onInputChange: (value: string) => void;
  /** Suggestions for the current input. Values already chosen are hidden. */
  items: readonly ComboItem[];
  emptyText: string;
  errorMessage?: string | undefined;
  mono?: boolean;
}

/** Search for values and collect them as a list of removable tags. */
export function MultiPickerField(props: MultiPickerFieldProps) {
  const chosen = new Set(props.value);
  const suggestions = props.items.filter((item) => !chosen.has(item.id));
  const text = props.mono ? "font-mono text-[13px]" : "";
  // Remount the search box after each pick so it starts closed and empty, then keep focus in it.
  const [round, setRound] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);
  useEffect(() => {
    if (round > 0) {
      inputRef.current?.focus();
    }
  }, [round]);

  return (
    <div className="flex flex-col gap-1.5">
      <RACComboBox
        key={round}
        inputValue={props.inputValue}
        onInputChange={props.onInputChange}
        items={suggestions}
        value={null}
        onChange={(key) => {
          if (key !== null && !chosen.has(String(key))) {
            props.onChange([...props.value, String(key)]);
          }
          props.onInputChange("");
          setRound((r) => r + 1);
        }}
        menuTrigger="input"
        allowsEmptyCollection
        isInvalid={Boolean(props.errorMessage)}
        validationBehavior="aria"
        className="flex flex-col gap-1.5"
      >
        <Label className={fieldLabel}>{props.label}</Label>
        <div className="relative">
          <Plus
            aria-hidden
            className="pointer-events-none absolute top-1/2 left-2.5 size-4 -translate-y-1/2 text-fg-muted"
          />
          <Input
            ref={inputRef}
            {...(props.placeholder ? { placeholder: props.placeholder } : {})}
            className={`${fieldInput} pl-8`}
          />
        </div>
        {props.description && (
          <Text slot="description" className={fieldDescription}>
            {props.description}
          </Text>
        )}
        <FieldError className={fieldError}>{props.errorMessage}</FieldError>
        <Popover className={popover} placement="bottom start">
          <ListBox
            className="outline-none"
            renderEmptyState={() => <Text className="block px-2 py-1.5 text-sm text-fg-muted">{props.emptyText}</Text>}
          >
            {(item: ComboItem) => (
              <ListBoxItem id={item.id} textValue={item.label} className={`${listItem} ${text}`}>
                {item.label}
              </ListBoxItem>
            )}
          </ListBox>
        </Popover>
      </RACComboBox>
      <TagGroup
        aria-label={`Selected ${props.label.toLowerCase()}`}
        onRemove={(keys) => props.onChange(props.value.filter((v) => !keys.has(v)))}
      >
        <TagList
          className="flex flex-wrap gap-1.5"
          renderEmptyState={() => <span className="text-xs text-fg-muted">None selected.</span>}
        >
          {props.value.map((v) => (
            <Tag
              key={v}
              id={v}
              textValue={v}
              className={`flex items-center gap-1 rounded-md border border-border bg-surface-hover py-0.5 pr-1 pl-2 text-fg ${text} ${focusRing}`}
            >
              {v}
              <RACButton
                slot="remove"
                aria-label="Remove"
                className={`rounded p-0.5 text-fg-muted hover:bg-border hover:text-fg ${focusRing}`}
              >
                <X aria-hidden className="size-3.5" />
              </RACButton>
            </Tag>
          ))}
        </TagList>
      </TagGroup>
    </div>
  );
}
