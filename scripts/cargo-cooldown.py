#!/usr/bin/env python3
"""Fail if any crate in Cargo.lock was published less than 72 hours ago.

Usage: python3 scripts/cargo-cooldown.py backend/Cargo.lock [hours]
"""

import datetime
import json
import re
import sys
import time
import urllib.request


def main() -> int:
    lock_path = sys.argv[1] if len(sys.argv) > 1 else "Cargo.lock"
    hours = int(sys.argv[2]) if len(sys.argv) > 2 else 72
    lock = open(lock_path, encoding="utf-8").read()
    pkgs = re.findall(
        r'\[\[package\]\]\nname = "([^"]+)"\nversion = "([^"]+)"\nsource = "registry', lock
    )
    cutoff = datetime.datetime.now(datetime.timezone.utc) - datetime.timedelta(hours=hours)
    recent = []
    for name, version in pkgs:
        req = urllib.request.Request(
            f"https://crates.io/api/v1/crates/{name}/{version}",
            headers={"User-Agent": "soabox-cooldown-check"},
        )
        data = json.load(urllib.request.urlopen(req, timeout=20))
        created = datetime.datetime.fromisoformat(data["version"]["created_at"].replace("Z", "+00:00"))
        if created > cutoff:
            recent.append(f"{name} {version} published {created.isoformat()}")
        time.sleep(0.05)
    print(f"{len(pkgs)} crates checked against a {hours} hour cooldown")
    for r in recent:
        print(f"too new: {r}")
    return 1 if recent else 0


if __name__ == "__main__":
    sys.exit(main())
