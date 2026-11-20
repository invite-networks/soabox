import { describe, expect, it } from "vitest";
import {
  ensureDot,
  isHostname,
  isInZone,
  isIPv4,
  isIPv6,
  isReverseZone,
  normalizeContent,
  toAbsoluteName,
  toRelativeName,
  validateContent,
  validateTtl,
  validateZoneName,
} from "./dns";

describe("names", () => {
  it("detects reverse zones", () => {
    expect(isReverseZone("2.0.192.in-addr.arpa.")).toBe(true);
    expect(isReverseZone("8.b.d.0.1.0.0.2.ip6.arpa.")).toBe(true);
    expect(isReverseZone("example.com.")).toBe(false);
  });

  it("converts between relative and absolute names", () => {
    expect(toAbsoluteName("@", "example.com.")).toBe("example.com.");
    expect(toAbsoluteName("", "example.com.")).toBe("example.com.");
    expect(toAbsoluteName("www", "example.com.")).toBe("www.example.com.");
    expect(toAbsoluteName("mail.example.com.", "example.com.")).toBe("mail.example.com.");
    expect(toRelativeName("example.com.", "example.com.")).toBe("@");
    expect(toRelativeName("a.b.example.com.", "example.com.")).toBe("a.b");
    expect(toRelativeName("other.net.", "example.com.")).toBe("other.net.");
    expect(isInZone("www.example.com.", "example.com.")).toBe(true);
    expect(isInZone("example.net.", "example.com.")).toBe(false);
    expect(ensureDot("a.b")).toBe("a.b.");
  });

  it("validates host names", () => {
    expect(isHostname("www.example.com.")).toBe(true);
    expect(isHostname("*.example.com")).toBe(true);
    expect(isHostname("_sip._tcp.example.com.")).toBe(true);
    expect(isHostname("a.*.example.com")).toBe(false);
    expect(isHostname("-bad.example.com")).toBe(false);
    expect(isHostname(".")).toBe(false);
    expect(isHostname(".", true)).toBe(true);
    expect(isHostname("")).toBe(false);
    expect(validateZoneName("example.com")).toBeNull();
    expect(validateZoneName("bad zone")).not.toBeNull();
  });

  it("validates addresses", () => {
    expect(isIPv4("192.0.2.1")).toBe(true);
    expect(isIPv4("256.0.0.1")).toBe(false);
    expect(isIPv6("2001:db8::1")).toBe(true);
    expect(isIPv6("::ffff:192.0.2.1")).toBe(true);
    expect(isIPv6("2001:db8::g")).toBe(false);
    expect(isIPv6("192.0.2.1")).toBe(false);
    expect(isIPv6("2001:db8:0:0:0:0:0:1")).toBe(true);
    expect(isIPv6("2001:db8:0:0:0:0:1")).toBe(false);
    expect(isIPv6("::")).toBe(true);
    expect(isIPv6("fe80::")).toBe(true);
    expect(isIPv6("1::2::3")).toBe(false);
    expect(isIPv6("::ffff:192.0.2.256")).toBe(false);
    expect(isIPv6("1:2:3:4:5:6:192.0.2.1")).toBe(true);
    expect(isIPv6("12345::1")).toBe(false);
  });
});

describe("record content", () => {
  it.each([
    ["A", "192.0.2.1", true],
    ["A", "192.0.2", false],
    ["AAAA", "2001:db8::1", true],
    ["AAAA", "192.0.2.1", false],
    ["CNAME", "target.example.com", true],
    ["CNAME", "not valid", false],
    ["NS", "ns1.example.com.", true],
    ["PTR", "host.example.com.", true],
    ["MX", "10 mail.example.com", true],
    ["MX", "0 .", true],
    ["MX", "mail.example.com", false],
    ["MX", "70000 mail.example.com", false],
    ["SRV", "10 5 5060 sip.example.com", true],
    ["SRV", "10 5 sip.example.com", false],
    ["TXT", "hello world", true],
    ["TXT", '"quoted" "parts"', true],
    ["TXT", '"unterminated', false],
    ["CAA", '0 issue "ca.example.net"', true],
    ["CAA", "0 issue ca.example.net", true],
    ["CAA", "300 issue ca.example.net", false],
    ["SOA", "ns1.example.com. hostmaster.example.com. 1 10800 3600 604800 3600", true],
    ["SOA", "ns1.example.com. 1 2 3", false],
    ["HINFO", '"PC" "Linux"', true],
    ["A", "", false],
  ])("%s %s is valid: %s", (type, content, valid) => {
    expect(validateContent(type, content) === null).toBe(valid);
  });

  it("normalizes content", () => {
    expect(normalizeContent("CNAME", "a.example.com")).toBe("a.example.com.");
    expect(normalizeContent("MX", "10 mail.example.com")).toBe("10 mail.example.com.");
    expect(normalizeContent("MX", "0 .")).toBe("0 .");
    expect(normalizeContent("SRV", "1 2 3 t.example.com")).toBe("1 2 3 t.example.com.");
    expect(normalizeContent("TXT", 'say "hi"')).toBe('"say \\"hi\\""');
    expect(normalizeContent("TXT", '"already"')).toBe('"already"');
    expect(normalizeContent("CAA", "0 issue ca.example.net")).toBe('0 issue "ca.example.net"');
    expect(normalizeContent("A", " 192.0.2.1 ")).toBe("192.0.2.1");
    expect(normalizeContent("CNAME", "")).toBe("");
  });

  it("splits long TXT values into 255 character strings", () => {
    const long = "x".repeat(600);
    const result = normalizeContent("TXT", long);
    expect(result.split('" "')).toHaveLength(3);
    expect(validateContent("TXT", long)).toBeNull();
    expect(normalizeContent("TXT", "")).toBe('""');
  });

  it("validates TTL", () => {
    expect(validateTtl(300)).toBeNull();
    expect(validateTtl(-1)).not.toBeNull();
    expect(validateTtl(1.5)).not.toBeNull();
  });
});
