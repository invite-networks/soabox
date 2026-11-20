import type { Key, ReactNode } from "react";
import { Tab as RACTab, TabList as RACTabList, TabPanel as RACTabPanel, Tabs as RACTabs } from "react-aria-components";
import { focusRing } from "./styles";

export interface TabsProps {
  selectedKey: string;
  onSelectionChange: (key: string) => void;
  children: ReactNode;
}

export function Tabs({ selectedKey, onSelectionChange, children }: TabsProps) {
  return (
    <RACTabs
      selectedKey={selectedKey}
      onSelectionChange={(key: Key) => onSelectionChange(String(key))}
      className="flex flex-col gap-4"
    >
      {children}
    </RACTabs>
  );
}

export function TabList({ label, children }: { label: string; children: ReactNode }) {
  return (
    <RACTabList aria-label={label} className="flex gap-1 overflow-x-auto border-b border-border">
      {children}
    </RACTabList>
  );
}

export function Tab({ id, children }: { id: string; children: ReactNode }) {
  return (
    <RACTab
      id={id}
      className={[
        "-mb-px flex cursor-default items-center gap-1.5 border-b-2 border-transparent px-3 py-2 text-sm whitespace-nowrap text-fg-muted [&_svg]:size-4",
        "hovered:text-fg selected:border-accent selected:font-medium selected:text-accent-text",
        focusRing,
      ].join(" ")}
    >
      {children}
    </RACTab>
  );
}

export function TabPanel({ id, children }: { id: string; children: ReactNode }) {
  return (
    <RACTabPanel id={id} className={`rounded-md ${focusRing}`}>
      {children}
    </RACTabPanel>
  );
}
