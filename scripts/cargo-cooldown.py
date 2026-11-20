#!/usr/bin/env python3
"""Fail if any crate in Cargo.lock was published less than 72 hours ago.

Usage: python3 scripts/cargo-cooldown.py backend/Cargo.lock [hours] [base Cargo.lock]

With a base lockfile, only versions that are new or changed relative to the
base are checked. crates.io allows about one request per second.
"""

import datetime
import json
import re
import sys
import time
import urllib.error
import urllib.request

# Versions allowed inside the cooldown, each with a reason. Remove entries
# once they are older than the cooldown.
EXCEPTIONS = {
    ("sqlx", "0.9.1"): "fixes RUSTSEC-2026-0459 through libsqlite3-sys 0.39",
    ("sqlx-core", "0.9.1"): "part of the sqlx 0.9.1 security update",
    ("sqlx-macros", "0.9.1"): "part of the sqlx 0.9.1 security update",
    ("sqlx-macros-core", "0.9.1"): "part of the sqlx 0.9.1 security update",
    ("sqlx-mysql", "0.9.1"): "part of the sqlx 0.9.1 security update",
    ("sqlx-postgres", "0.9.1"): "part of the sqlx 0.9.1 security update",
    ("sqlx-sqlite", "0.9.1"): "part of the sqlx 0.9.1 security update",
    ("libsqlite3-sys", "0.39.0"): "fixes RUSTSEC-2026-0459 (SQLite CVE-2026-119375)",
}


def main() -> int:
    lock_path = sys.argv[1] if len(sys.argv) > 1 else "Cargo.lock"
    hours = int(sys.argv[2]) if len(sys.argv) > 2 else 72
    pattern = (
        r'\[\[package\]\]\nname = "([^"]+)"\nversion = "([^"]+)"\nsource = "registry'
    )
    pkgs = re.findall(pattern, open(lock_path, encoding="utf-8").read())
    if len(sys.argv) > 3:
        base = set(re.findall(pattern, open(sys.argv[3], encoding="utf-8").read()))
        pkgs = [p for p in pkgs if p not in base]
    cutoff = datetime.datetime.now(datetime.timezone.utc) - datetime.timedelta(
        hours=hours
    )
    recent = []
    for name, version in pkgs:
        req = urllib.request.Request(
            f"https://crates.io/api/v1/crates/{name}/{version}",
            headers={"User-Agent": "soabox-cooldown-check"},
        )
        for attempt in range(5):
            try:
                data = json.load(urllib.request.urlopen(req, timeout=20))
                break
            except urllib.error.HTTPError as e:
                if e.code != 429 or attempt == 4:
                    raise
                time.sleep(5 * (attempt + 1))
        created = datetime.datetime.fromisoformat(
            data["version"]["created_at"].replace("Z", "+00:00")
        )
        if created > cutoff and (name, version) not in EXCEPTIONS:
            recent.append(f"{name} {version} published {created.isoformat()}")
        time.sleep(1)
    print(f"{len(pkgs)} crates checked against a {hours} hour cooldown")
    for r in recent:
        print(f"too new: {r}")
    return 1 if recent else 0


if __name__ == "__main__":
    sys.exit(main())
