import { useNavigate } from "@tanstack/react-router";
import { useEffect, useState } from "react";
import { useZones } from "../api/hooks/zones";
import { QuickSearch } from "./ui";

/** Top bar search that jumps straight to a zone. */
export function ZoneQuickSearch() {
  const navigate = useNavigate();
  const [input, setInput] = useState("");
  const [query, setQuery] = useState("");

  useEffect(() => {
    const timer = window.setTimeout(() => setQuery(input.trim()), 200);
    return () => window.clearTimeout(timer);
  }, [input]);

  const zones = useZones({ q: query, per_page: 8 }, query.length > 0);
  const items = (query.length > 0 ? (zones.data?.items ?? []) : []).map((zone) => ({ id: zone.id, label: zone.name }));

  return (
    <QuickSearch
      label="Find a zone"
      placeholder="Find a zone"
      inputValue={input}
      onInputChange={setInput}
      items={items}
      emptyText={zones.isFetching ? "Searching" : "No matching zones"}
      onSelect={(id) => {
        setInput("");
        void navigate({ to: "/zones/$zoneId", params: { zoneId: id } });
      }}
    />
  );
}
