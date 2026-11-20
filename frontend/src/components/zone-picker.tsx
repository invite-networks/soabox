import { useEffect, useState } from "react";
import { useZones } from "../api/hooks/zones";
import { MultiPickerField } from "./ui";

export interface ZonePickerFieldProps {
  label?: string;
  description?: string;
  value: readonly string[];
  onChange: (value: string[]) => void;
}

/** Pick zones that exist on the server and collect them as a list. */
export function ZonePickerField({ label = "Zones", description, value, onChange }: ZonePickerFieldProps) {
  const [input, setInput] = useState("");
  const [query, setQuery] = useState("");

  useEffect(() => {
    const timer = window.setTimeout(() => setQuery(input.trim()), 200);
    return () => window.clearTimeout(timer);
  }, [input]);

  const zones = useZones({ q: query, per_page: 20 });
  const items = (zones.data?.items ?? []).map((zone) => ({ id: zone.name, label: zone.name }));

  return (
    <MultiPickerField
      label={label}
      {...(description ? { description } : {})}
      placeholder="Search zones to add"
      value={value}
      onChange={onChange}
      inputValue={input}
      onInputChange={setInput}
      items={items}
      emptyText={zones.isFetching ? "Searching" : "No matching zones"}
      mono
    />
  );
}
