import type { Ref } from "react";
import {
  FieldError,
  Input,
  Label,
  TextArea as RACTextArea,
  TextField as RACTextField,
  Text,
} from "react-aria-components";
import { fieldDescription, fieldError, fieldInput, fieldLabel } from "./styles";

interface BaseFieldProps {
  label: string;
  value: string;
  onChange: (value: string) => void;
  onBlur?: () => void;
  name?: string;
  description?: string;
  errorMessage?: string | undefined;
  isRequired?: boolean;
  isDisabled?: boolean;
  isReadOnly?: boolean;
  placeholder?: string;
  autoComplete?: string;
  autoFocus?: boolean;
  /** Use the monospace font for technical values. */
  mono?: boolean;
  /** Hide the label visually. It stays available to assistive technology. */
  hideLabel?: boolean;
}

export interface TextFieldProps extends BaseFieldProps {
  type?: "text" | "password" | "email" | "number" | "date" | "search" | "url";
  inputMode?: "text" | "numeric";
  inputRef?: Ref<HTMLInputElement>;
}

function common(props: BaseFieldProps) {
  return {
    value: props.value,
    onChange: props.onChange,
    ...(props.onBlur ? { onBlur: props.onBlur } : {}),
    ...(props.name ? { name: props.name } : {}),
    ...(props.autoComplete ? { autoComplete: props.autoComplete } : {}),
    ...(props.autoFocus ? { autoFocus: true } : {}),
    isInvalid: Boolean(props.errorMessage),
    isRequired: props.isRequired ?? false,
    isDisabled: props.isDisabled ?? false,
    isReadOnly: props.isReadOnly ?? false,
    validationBehavior: "aria" as const,
    className: "flex flex-col gap-1",
  };
}

function FieldLabel({ label, hideLabel }: { label: string; hideLabel?: boolean | undefined }) {
  return <Label className={hideLabel ? "sr-only" : fieldLabel}>{label}</Label>;
}

/** A labeled single-line text input with description and error message. */
export function TextField(props: TextFieldProps) {
  return (
    <RACTextField
      {...common(props)}
      type={props.type ?? "text"}
      {...(props.inputMode ? { inputMode: props.inputMode } : {})}
    >
      <FieldLabel label={props.label} hideLabel={props.hideLabel} />
      <Input
        {...(props.inputRef ? { ref: props.inputRef } : {})}
        {...(props.placeholder ? { placeholder: props.placeholder } : {})}
        className={`${fieldInput} ${props.mono ? "font-mono" : ""}`}
      />
      {props.description && (
        <Text slot="description" className={fieldDescription}>
          {props.description}
        </Text>
      )}
      <FieldError className={fieldError}>{props.errorMessage}</FieldError>
    </RACTextField>
  );
}

export interface TextAreaFieldProps extends BaseFieldProps {
  rows?: number;
  inputRef?: Ref<HTMLTextAreaElement>;
}

/** A labeled multi-line text input. */
export function TextAreaField(props: TextAreaFieldProps) {
  return (
    <RACTextField {...common(props)}>
      <FieldLabel label={props.label} hideLabel={props.hideLabel} />
      <RACTextArea
        {...(props.inputRef ? { ref: props.inputRef } : {})}
        {...(props.placeholder ? { placeholder: props.placeholder } : {})}
        rows={props.rows ?? 3}
        className={`${fieldInput} ${props.mono ? "font-mono" : ""} resize-y`}
      />
      {props.description && (
        <Text slot="description" className={fieldDescription}>
          {props.description}
        </Text>
      )}
      <FieldError className={fieldError}>{props.errorMessage}</FieldError>
    </RACTextField>
  );
}
