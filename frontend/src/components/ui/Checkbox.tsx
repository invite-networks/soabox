import { Check, Minus } from "lucide-react";
import type { ReactNode } from "react";
import {
  FieldError,
  Label,
  Checkbox as RACCheckbox,
  CheckboxGroup as RACCheckboxGroup,
  Switch as RACSwitch,
  Text,
} from "react-aria-components";
import { fieldDescription, fieldError, fieldLabel } from "./styles";

export interface CheckboxProps {
  children: ReactNode;
  isSelected?: boolean;
  onChange?: (selected: boolean) => void;
  value?: string;
  isDisabled?: boolean;
}

const box = [
  "flex size-4 shrink-0 items-center justify-center rounded border border-border-strong bg-surface text-accent-fg",
  "group-selected:border-accent group-selected:bg-accent group-indeterminate:bg-accent",
  "group-focus-visible:outline-2 group-focus-visible:outline-offset-2 group-focus-visible:outline-focus",
].join(" ");

/** A checkbox with a label. */
export function Checkbox({ children, isSelected, onChange, value, isDisabled }: CheckboxProps) {
  return (
    <RACCheckbox
      {...(isSelected !== undefined ? { isSelected } : {})}
      {...(onChange ? { onChange } : {})}
      {...(value !== undefined ? { value } : {})}
      isDisabled={isDisabled ?? false}
      className="group flex items-center gap-2 text-sm text-fg disabled:opacity-50"
    >
      {({ isSelected: selected, isIndeterminate }) => (
        <>
          <span className={box}>
            {isIndeterminate ? (
              <Minus aria-hidden className="size-3" />
            ) : selected ? (
              <Check aria-hidden className="size-3" />
            ) : null}
          </span>
          {children}
        </>
      )}
    </RACCheckbox>
  );
}

export interface CheckboxGroupFieldProps {
  label: string;
  value: string[];
  onChange: (value: string[]) => void;
  options: readonly { id: string; label: string }[];
  description?: string;
  errorMessage?: string | undefined;
  columns?: 1 | 2 | 3 | 4;
}

const columnClasses = {
  1: "grid-cols-1",
  2: "grid-cols-2",
  3: "grid-cols-2 sm:grid-cols-3",
  4: "grid-cols-2 sm:grid-cols-4",
} as const;

/** A labeled group of checkboxes for picking several values. */
export function CheckboxGroupField(props: CheckboxGroupFieldProps) {
  return (
    <RACCheckboxGroup
      value={props.value}
      onChange={props.onChange}
      isInvalid={Boolean(props.errorMessage)}
      validationBehavior="aria"
      className="flex flex-col gap-1.5"
    >
      <Label className={fieldLabel}>{props.label}</Label>
      {props.options.length === 0 ? (
        <Text slot="description" className={fieldDescription}>
          Nothing to choose from yet.
        </Text>
      ) : (
        <div className={`grid gap-x-4 gap-y-1.5 ${columnClasses[props.columns ?? 2]}`}>
          {props.options.map((option) => (
            <Checkbox key={option.id} value={option.id}>
              {option.label}
            </Checkbox>
          ))}
        </div>
      )}
      {props.description && (
        <Text slot="description" className={fieldDescription}>
          {props.description}
        </Text>
      )}
      <FieldError className={fieldError}>{props.errorMessage}</FieldError>
    </RACCheckboxGroup>
  );
}

export interface SwitchProps {
  children: ReactNode;
  isSelected: boolean;
  onChange: (selected: boolean) => void;
  isDisabled?: boolean;
  description?: string;
}

/** An on and off switch with a label. */
export function Switch({ children, isSelected, onChange, isDisabled, description }: SwitchProps) {
  return (
    <div className="flex flex-col gap-0.5">
      <RACSwitch
        isSelected={isSelected}
        onChange={onChange}
        isDisabled={isDisabled ?? false}
        className="group flex items-center gap-2.5 text-sm text-fg disabled:opacity-50"
      >
        <span className="flex h-5 w-9 shrink-0 items-center rounded-full border border-border-strong bg-surface-muted px-0.5 transition-colors group-selected:border-accent group-selected:bg-accent group-focus-visible:outline-2 group-focus-visible:outline-offset-2 group-focus-visible:outline-focus">
          <span className="size-3.5 rounded-full bg-fg-muted transition-transform group-selected:translate-x-4 group-selected:bg-accent-fg" />
        </span>
        {children}
      </RACSwitch>
      {description && <span className={`${fieldDescription} pl-11.5`}>{description}</span>}
    </div>
  );
}
