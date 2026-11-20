import { describe, expect, it } from "vitest";
import type { RrSet } from "../api/types";
import {
  changeStatus,
  compareRrSets,
  rrKey,
  toPatchRequest,
  withChange,
  withoutChange,
  workingRrSets,
} from "./pending";

const www: RrSet = {
  name: "www.example.com.",
  type: "A",
  ttl: 300,
  records: [{ content: "192.0.2.1", disabled: false }],
  comments: [],
};
const apexNs: RrSet = {
  name: "example.com.",
  type: "NS",
  ttl: 3600,
  records: [{ content: "ns1.example.com.", disabled: false }],
  comments: [],
};
const apexSoa: RrSet = {
  name: "example.com.",
  type: "SOA",
  ttl: 3600,
  records: [{ content: "x", disabled: false }],
  comments: [],
};

describe("pending changes", () => {
  it("adds, changes, and deletes RRsets", () => {
    const added: RrSet = { ...www, name: "api.example.com." };
    let changes = withChange(new Map(), null, added);
    changes = withChange(changes, www, { ...www, ttl: 60 });
    changes = withChange(changes, apexNs, null);
    expect([...changes.values()].map(changeStatus)).toEqual(["added", "changed", "deleted"]);

    const rows = workingRrSets([apexSoa, apexNs, www], changes);
    expect(rows.map((r) => `${r.rrset.name} ${r.rrset.type} ${r.status ?? "-"}`)).toEqual([
      "example.com. SOA -",
      "example.com. NS deleted",
      "api.example.com. A added",
      "www.example.com. A changed",
    ]);

    expect(toPatchRequest(changes)).toEqual({
      rrsets: [
        { name: "example.com.", type: "NS", ttl: 3600, changetype: "DELETE", records: [] },
        {
          name: "api.example.com.",
          type: "A",
          ttl: 300,
          changetype: "REPLACE",
          records: [{ content: "192.0.2.1", disabled: false }],
        },
        {
          name: "www.example.com.",
          type: "A",
          ttl: 60,
          changetype: "REPLACE",
          records: [{ content: "192.0.2.1", disabled: false }],
        },
      ],
    });
  });

  it("drops a change that restores the server state", () => {
    let changes = withChange(new Map(), www, { ...www, ttl: 60 });
    changes = withChange(changes, www, { ...www });
    expect(changes.size).toBe(0);
    expect(withChange(new Map(), null, null).size).toBe(0);
    changes = withChange(new Map(), www, null);
    expect(withoutChange(changes, rrKey(www.name, www.type)).size).toBe(0);
  });

  it("sorts apex first with SOA and NS leading", () => {
    const list = [www, apexNs, apexSoa, { ...www, type: "AAAA" }];
    expect(list.sort(compareRrSets).map((r) => r.type)).toEqual(["SOA", "NS", "A", "AAAA"]);
  });
});
