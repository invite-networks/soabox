import { X } from "lucide-react";
import type { ReactNode } from "react";
import { Heading, Modal, ModalOverlay, Dialog as RACDialog } from "react-aria-components";
import { Button } from "./Button";

export interface DialogProps {
  title: string;
  isOpen: boolean;
  onOpenChange: (isOpen: boolean) => void;
  children: ReactNode;
  size?: "sm" | "md" | "lg" | "xl";
  /** Use `alertdialog` for destructive confirmations. */
  role?: "dialog" | "alertdialog";
  isDismissable?: boolean;
}

const widths = {
  sm: "max-w-sm",
  md: "max-w-lg",
  lg: "max-w-2xl",
  xl: "max-w-4xl",
} as const;

/** A modal dialog with a title and a close button. */
export function Dialog({
  title,
  isOpen,
  onOpenChange,
  children,
  size = "md",
  role = "dialog",
  isDismissable = true,
}: DialogProps) {
  return (
    <ModalOverlay
      isOpen={isOpen}
      onOpenChange={onOpenChange}
      isDismissable={isDismissable}
      className="fixed inset-0 z-50 flex items-start justify-center overflow-y-auto bg-overlay p-4 sm:items-center"
    >
      <Modal className={`w-full ${widths[size]} rounded-lg border border-border bg-surface shadow-xl`}>
        <RACDialog role={role} className="flex max-h-[90vh] flex-col outline-none">
          <div className="flex items-center justify-between gap-4 border-b border-border px-4 py-3">
            <Heading slot="title" className="text-base font-semibold text-fg">
              {title}
            </Heading>
            <Button variant="ghost" size="icon" aria-label="Close" onPress={() => onOpenChange(false)}>
              <X aria-hidden />
            </Button>
          </div>
          <div className="overflow-y-auto px-4 py-4">{children}</div>
        </RACDialog>
      </Modal>
    </ModalOverlay>
  );
}

/** Buttons at the bottom of a dialog. */
export function DialogFooter({ children }: { children: ReactNode }) {
  return <div className="mt-5 flex flex-wrap justify-end gap-2">{children}</div>;
}

export interface DrawerProps {
  isOpen: boolean;
  onOpenChange: (isOpen: boolean) => void;
  label: string;
  children: ReactNode;
}

/** A panel that slides in from the left. Used for navigation on small screens. */
export function Drawer({ isOpen, onOpenChange, label, children }: DrawerProps) {
  return (
    <ModalOverlay isOpen={isOpen} onOpenChange={onOpenChange} isDismissable className="fixed inset-0 z-50 bg-overlay">
      <Modal className="h-full w-64 max-w-[85vw] border-r border-border bg-surface shadow-xl">
        <RACDialog aria-label={label} className="flex h-full flex-col outline-none">
          {children}
        </RACDialog>
      </Modal>
    </ModalOverlay>
  );
}
