import { ChevronDown } from "lucide-react";
import {
  Button,
  FieldError,
  Label,
  ListBox,
  ListBoxItem,
  Popover,
  Select as RACSelect,
  SelectValue,
  Text,
} from "react-aria-components";
import { fieldDescription, fieldError, fieldInput, fieldLabel, listItem, popover } from "./styles";

export interface SelectOption {
  id: string;
  label: string;
}

export interface SelectFieldProps {
  label: string;
  options: readonly SelectOption[];
  value: string | null;
  onChange: (value: string) => void;
  name?: string;
  description?: string;
  errorMessage?: string | undefined;
  placeholder?: string;
  isRequired?: boolean;
  isDisabled?: boolean;
  hideLabel?: boolean;
}

/** A labeled single choice list. */
export function SelectField(props: SelectFieldProps) {
  return (
    <RACSelect
      value={props.value}
      onChange={(key) => {
        if (key !== null) {
          props.onChange(String(key));
        }
      }}
      {...(props.name ? { name: props.name } : {})}
      {...(props.placeholder ? { placeholder: props.placeholder } : {})}
      isInvalid={Boolean(props.errorMessage)}
      isRequired={props.isRequired ?? false}
      isDisabled={props.isDisabled ?? false}
      validationBehavior="aria"
      className="flex flex-col gap-1"
    >
      <Label className={props.hideLabel ? "sr-only" : fieldLabel}>{props.label}</Label>
      <Button className={`${fieldInput} flex items-center justify-between gap-2 text-left`}>
        <SelectValue className="truncate placeholder-shown:text-fg-muted" />
        <ChevronDown aria-hidden className="size-4 text-fg-muted" />
      </Button>
      {props.description && (
        <Text slot="description" className={fieldDescription}>
          {props.description}
        </Text>
      )}
      <FieldError className={fieldError}>{props.errorMessage}</FieldError>
      <Popover className={popover}>
        <ListBox items={props.options} className="outline-none">
          {(item) => (
            <ListBoxItem id={item.id} textValue={item.label} className={listItem}>
              {item.label}
            </ListBoxItem>
          )}
        </ListBox>
      </Popover>
    </RACSelect>
  );
}
