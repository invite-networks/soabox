#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.13"
# dependencies = []
# ///
"""Load sample zones into a SOAbox demo or development stack.

Everything goes through the PowerDNS-compatible API, the same way an existing
PowerDNS client would, so each change is checked and recorded in history.
All names and addresses use reserved documentation ranges (RFC 2606, 5737, 3849).

Usage:
    SOABOX_URL=http://localhost:18080 SOABOX_API_KEY=... uv run scripts/seed-demo.py

Create an Admin key first, for example:
    docker compose exec soabox soabox apikey create --role admin --description demo-seed
"""

from __future__ import annotations

import json
import os
import sys
import urllib.error
import urllib.request
from typing import Any

BASE = os.environ.get("SOABOX_URL", "http://localhost:8080").rstrip("/")
KEY = os.environ.get("SOABOX_API_KEY", "")
API = f"{BASE}/api/v1/servers/localhost"

Record = tuple[str, str, int, list[str]]


def call(method: str, path: str, body: Any | None = None) -> tuple[int, Any]:
    data = json.dumps(body).encode() if body is not None else None
    req = urllib.request.Request(f"{API}{path}", data=data, method=method)
    req.add_header("X-API-Key", KEY)
    if data is not None:
        req.add_header("Content-Type", "application/json")
    try:
        with urllib.request.urlopen(req, timeout=30) as resp:
            raw = resp.read()
            return resp.status, json.loads(raw) if raw else None
    except urllib.error.HTTPError as err:
        raw = err.read()
        return err.code, json.loads(raw) if raw else None


def rrsets(zone: str, records: list[Record]) -> list[dict[str, Any]]:
    out = []
    for name, rtype, ttl, contents in records:
        fqdn = zone if name == "@" else f"{name}.{zone}"
        out.append(
            {
                "name": fqdn,
                "type": rtype,
                "ttl": ttl,
                "changetype": "REPLACE",
                "records": [{"content": c, "disabled": False} for c in contents],
            }
        )
    return out


def web_zone(zone: str, v4: str, v6: str, extra: list[Record]) -> list[Record]:
    return [
        ("@", "A", 300, [v4]),
        ("@", "AAAA", 300, [v6]),
        ("www", "CNAME", 3600, [zone]),
        ("@", "MX", 3600, [f"10 mx1.{zone}", f"20 mx2.{zone}"]),
        ("mx1", "A", 3600, ["192.0.2.25"]),
        ("mx2", "A", 3600, ["198.51.100.25"]),
        ("@", "TXT", 3600, ['"v=spf1 mx -all"']),
        (
            "_dmarc",
            "TXT",
            3600,
            [f'"v=DMARC1; p=quarantine; rua=mailto:dmarc@{zone[:-1]}"'],
        ),
        (
            "@",
            "CAA",
            86400,
            ['0 issue "letsencrypt.org"', f'0 iodef "mailto:security@{zone[:-1]}"'],
        ),
        *extra,
    ]


ZONES: list[dict[str, Any]] = [
    {
        "name": "example.com.",
        "kind": "Native",
        "account": "platform",
        "records": web_zone(
            "example.com.",
            "192.0.2.10",
            "2001:db8::10",
            [
                ("api", "A", 60, ["192.0.2.20", "192.0.2.21"]),
                ("api", "AAAA", 60, ["2001:db8::20", "2001:db8::21"]),
                ("status", "CNAME", 3600, ["status.example.net."]),
                ("vpn", "A", 3600, ["203.0.113.5"]),
                ("_sip._tcp", "SRV", 3600, ["10 60 5060 sip.example.com."]),
                ("sip", "A", 3600, ["192.0.2.60"]),
                ("_acme-challenge", "TXT", 60, ['"q9fJ2aXo3sE1rM0dYk7uTnBvLw4pZc8H"']),
                ("staging", "A", 300, ["198.51.100.40"]),
                ("legacy", "A", 300, ["198.51.100.41"]),
            ],
        ),
    },
    {
        "name": "example.net.",
        "kind": "Master",
        "account": "platform",
        "dnssec": True,
        "records": web_zone(
            "example.net.",
            "198.51.100.10",
            "2001:db8:1::10",
            [
                ("status", "A", 300, ["198.51.100.80"]),
                ("cdn", "CNAME", 300, ["cdn.example.org."]),
                ("@", "HTTPS", 3600, ["1 . alpn=h2,h3"]),
            ],
        ),
    },
    {
        "name": "example.org.",
        "kind": "Master",
        "account": "marketing",
        "records": web_zone(
            "example.org.",
            "203.0.113.10",
            "2001:db8:2::10",
            [
                ("blog", "CNAME", 3600, ["example.org."]),
                ("shop", "A", 300, ["203.0.113.30"]),
                ("cdn", "A", 300, ["203.0.113.40", "203.0.113.41"]),
                ("news", "CNAME", 3600, ["blog.example.org."]),
            ],
        ),
    },
    {
        "name": "corp.example.",
        "kind": "Native",
        "account": "it",
        "records": [
            ("@", "A", 3600, ["10.20.0.10"]),
            ("dc1", "A", 3600, ["10.20.0.11"]),
            ("dc2", "A", 3600, ["10.20.0.12"]),
            ("files", "A", 3600, ["10.20.1.20"]),
            ("print", "A", 3600, ["10.20.1.30"]),
            ("wiki", "CNAME", 3600, ["files.corp.example."]),
            (
                "_ldap._tcp",
                "SRV",
                600,
                ["0 100 389 dc1.corp.example.", "0 100 389 dc2.corp.example."],
            ),
            (
                "_kerberos._udp",
                "SRV",
                600,
                ["0 100 88 dc1.corp.example.", "0 100 88 dc2.corp.example."],
            ),
            ("@", "TXT", 3600, ['"internal zone, do not delegate"']),
        ],
    },
    {
        "name": "2.0.192.in-addr.arpa.",
        "kind": "Native",
        "account": "platform",
        "records": [
            ("10", "PTR", 3600, ["example.com."]),
            ("20", "PTR", 3600, ["api.example.com."]),
            ("21", "PTR", 3600, ["api.example.com."]),
            ("25", "PTR", 3600, ["mx1.example.com."]),
            ("60", "PTR", 3600, ["sip.example.com."]),
        ],
    },
    {
        "name": "partner.example.",
        "kind": "Slave",
        "masters": ["192.0.2.53"],
        "records": [],
    },
]


def main() -> int:
    if not KEY:
        print("Set SOABOX_API_KEY to an Admin API key.", file=sys.stderr)
        return 2
    for zone in ZONES:
        name = zone["name"]
        status, _ = call("GET", f"/zones/{name}")
        if status == 200:
            print(f"skip   {name} (exists)")
            continue
        body: dict[str, Any] = {
            "name": name,
            "kind": zone["kind"],
            "nameservers": []
            if zone["kind"] == "Slave"
            else ["ns1.example.com.", "ns2.example.net."],
            "masters": zone.get("masters", []),
            "account": zone.get("account", ""),
        }
        status, resp = call("POST", "/zones", body)
        if status != 201:
            print(f"fail   {name}: {status} {resp}", file=sys.stderr)
            return 1
        if zone["records"]:
            status, resp = call(
                "PATCH", f"/zones/{name}", {"rrsets": rrsets(name, zone["records"])}
            )
            if status != 204:
                print(f"fail   {name} records: {status} {resp}", file=sys.stderr)
                return 1
        if zone.get("dnssec"):
            status, resp = call(
                "POST",
                f"/zones/{name}/cryptokeys",
                {"keytype": "csk", "active": True, "algorithm": "ecdsap256sha256"},
            )
            if status != 201:
                print(f"warn   {name} DNSSEC: {status} {resp}", file=sys.stderr)
        print(f"create {name} ({len(zone['records'])} RRsets)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
