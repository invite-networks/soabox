import { type ReactNode, useState } from "react";
import { errorMessage } from "../api/client";
import { Alert, Button, Dialog, DialogFooter, TextField } from "./ui";

export interface ConfirmDialogProps {
  title: string;
  isOpen: boolean;
  onOpenChange: (isOpen: boolean) => void;
  onConfirm: () => void;
  confirmLabel: string;
  children: ReactNode;
  isPending?: boolean;
  error?: unknown;
  /** When set, the user must type this text to enable the confirm button. */
  requireText?: string;
  tone?: "danger" | "primary";
}

/** Asks the user to confirm an action. Destructive actions use the danger tone. */
export function ConfirmDialog(props: ConfirmDialogProps) {
  const [typed, setTyped] = useState("");
  const tone = props.tone ?? "danger";
  const matches = props.requireText === undefined || typed.trim() === props.requireText;
  return (
    <Dialog
      title={props.title}
      isOpen={props.isOpen}
      onOpenChange={(open) => {
        if (!open) {
          setTyped("");
        }
        props.onOpenChange(open);
      }}
      size="sm"
      role={tone === "danger" ? "alertdialog" : "dialog"}
    >
      <div className="flex flex-col gap-3 text-sm text-fg">
        {props.children}
        {props.requireText !== undefined && (
          <TextField
            label={`Type ${props.requireText} to confirm`}
            value={typed}
            onChange={setTyped}
            mono
            autoComplete="off"
          />
        )}
        {props.error ? <Alert tone="danger">{errorMessage(props.error)}</Alert> : null}
      </div>
      <DialogFooter>
        <Button onPress={() => props.onOpenChange(false)}>Cancel</Button>
        <Button variant={tone} isDisabled={!matches} isPending={props.isPending ?? false} onPress={props.onConfirm}>
          {props.confirmLabel}
        </Button>
      </DialogFooter>
    </Dialog>
  );
}
