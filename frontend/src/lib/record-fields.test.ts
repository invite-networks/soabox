import { describe, expect, it } from "vitest";
import {
  absoluteHost,
  composeFields,
  displayParts,
  fieldsFor,
  mailboxToEmail,
  parseFields,
  RECORD_FIELDS,
  restrictInput,
  validateField,
} from "./record-fields";

const zone = "example.com.";

describe("record fields", () => {
  it("defines fields for every known type", () => {
    for (const type of ["A", "AAAA", "CAA", "CNAME", "DS", "MX", "NAPTR", "SOA", "SRV", "SSHFP", "TLSA", "TXT"]) {
      expect(RECORD_FIELDS[type]?.length).toBeGreaterThan(0);
    }
    expect(fieldsFor("UNKNOWN")[0]?.label).toBe("Value");
  });

  it("resolves host names with zone file rules", () => {
    expect(absoluteHost("www", zone)).toBe("www.example.com.");
    expect(absoluteHost("@", zone)).toBe("example.com.");
    expect(absoluteHost("test.cname.com", zone)).toBe("test.cname.com.example.com.");
    expect(absoluteHost("test.cname.com.", zone)).toBe("test.cname.com.");
    expect(absoluteHost("mail", null)).toBe("mail");
    expect(absoluteHost("hostmaster@example.net", zone, true)).toBe("hostmaster.example.net.");
    expect(absoluteHost("first.last@example.net", zone, true)).toBe("first\\.last.example.net.");
  });

  it("round trips stored content through the fields", () => {
    const cases: [string, string, string[]][] = [
      ["MX", "10 mail.example.com.", ["10", "mail"]],
      ["MX", "0 .", ["0", "."]],
      ["SRV", "10 5 5060 sip.other.net.", ["10", "5", "5060", "sip.other.net."]],
      ["CAA", '0 issue "letsencrypt.org"', ["0", "issue", "letsencrypt.org"]],
      ["TXT", '"v=spf1 -all"', ["v=spf1 -all"]],
      ["HTTPS", "1 . alpn=h2,h3", ["1", ".", "alpn=h2,h3"]],
      ["CNAME", "example.com.", ["@"]],
      [
        "SOA",
        "ns1.example.com. hostmaster.example.com. 1 10800 3600 604800 3600",
        ["ns1", "hostmaster@example.com", "1", "10800", "3600", "604800", "3600"],
      ],
      ["NAPTR", '100 10 "U" "E2U+sip" "!^.*$!sip:a@b!" .', ["100", "10", "U", "E2U+sip", "!^.*$!sip:a@b!", "."]],
    ];
    for (const [type, content, fields] of cases) {
      expect(parseFields(type, content, zone)).toEqual(fields);
      expect(composeFields(type, fields, zone)).toBe(content);
    }
  });

  it("joins long TXT values and splits them again", () => {
    const long = "a".repeat(300);
    const stored = composeFields("TXT", [long], zone);
    expect(stored).toBe(`"${"a".repeat(255)}" "${"a".repeat(45)}"`);
    expect(parseFields("TXT", stored, zone)).toEqual([long]);
  });

  it("shows mailboxes as email addresses", () => {
    expect(mailboxToEmail("hostmaster.corp.example.")).toBe("hostmaster@corp.example");
    expect(mailboxToEmail("first\\.last.example.net.")).toBe("first.last@example.net");
    expect(mailboxToEmail(".")).toBe(".");
    expect(composeFields("RP", ["admin@example.net", "."], zone)).toBe("admin.example.net. .");
    expect(parseFields("RP", "admin.example.net. .", zone)).toEqual(["admin@example.net", "."]);
  });

  it("returns null for content the fields cannot hold", () => {
    expect(parseFields("MX", "10", zone)).toBeNull();
    expect(parseFields("MX", "10 a b", zone)).toBeNull();
    expect(parseFields("TXT", "unquoted", zone)).toBeNull();
    expect(parseFields("UNKNOWN", "x", zone)).toBeNull();
  });

  it("restricts and validates input", () => {
    const [priority, target] = fieldsFor("MX");
    expect(priority && restrictInput(priority, "1a0")).toBe("10");
    expect(target && restrictInput(target, "ma il!")).toBe("mail");
    expect(priority && validateField(priority, "70000", zone)).toBe("Enter a number from 0 to 65535.");
    expect(target && validateField(target, ".", zone)).toBeNull();
    expect(target && validateField(target, "", zone)).toBe("Mail server is required.");
    const [cname] = fieldsFor("CNAME");
    expect(cname && validateField(cname, ".", zone)).toBe("Enter a host name.");
  });

  it("marks the zone suffix for display", () => {
    expect(displayParts("CNAME", "test.cname.com.example.com.", zone)).toEqual([
      { text: "test.cname.com" },
      { text: ".example.com.", suffix: true },
    ]);
    expect(displayParts("CNAME", "test.cname.com.", zone)).toEqual([{ text: "test.cname.com." }]);
    expect(displayParts("MX", "10 example.com.", zone)).toEqual([
      { text: "10 " },
      { text: "example.com.", suffix: true },
    ]);
    expect(displayParts("A", "192.0.2.1", zone)).toEqual([{ text: "192.0.2.1" }]);
    expect(displayParts("SOA", "ns1.example.com. hostmaster.example.com. 1 2 3 4 5", zone)).toEqual([
      { text: "ns1" },
      { text: ".example.com.", suffix: true },
      { text: " hostmaster.example.com. 1 2 3 4 5" },
    ]);
  });
});
