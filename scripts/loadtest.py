#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.13"
# dependencies = ["httpx>=0.28"]
# ///
"""Measure the latency SOAbox adds to PowerDNS API calls (NFR-PERF-01, 02).

Sends requests at a fixed rate to PowerDNS directly and through SOAbox and
compares the 95th percentile latency.

Usage:
  uv run scripts/loadtest.py --pdns http://localhost:8081 --pdns-key K \\
      --soabox http://localhost:8080 --soabox-key S [--path /zones] [--rate 50] [--seconds 30]
"""

import argparse
import asyncio
import statistics
import time

import httpx


async def run(base: str, key: str, path: str, rate: int, seconds: int) -> list[float]:
    latencies: list[float] = []
    errors = 0
    async with httpx.AsyncClient(
        base_url=base, headers={"X-API-Key": key}, timeout=10
    ) as client:

        async def one() -> None:
            nonlocal errors
            start = time.perf_counter()
            resp = await client.get(f"/api/v1/servers/localhost{path}")
            latencies.append((time.perf_counter() - start) * 1000)
            if resp.status_code != 200:
                errors += 1

        tasks = []
        interval = 1 / rate
        t0 = time.perf_counter()
        for i in range(rate * seconds):
            delay = t0 + i * interval - time.perf_counter()
            if delay > 0:
                await asyncio.sleep(delay)
            tasks.append(asyncio.create_task(one()))
        await asyncio.gather(*tasks)
    if errors:
        raise SystemExit(f"{base}: {errors} non-200 responses")
    return latencies


def p95(values: list[float]) -> float:
    return statistics.quantiles(values, n=100)[94]


async def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--pdns", required=True)
    ap.add_argument("--pdns-key", required=True)
    ap.add_argument("--soabox", required=True)
    ap.add_argument("--soabox-key", required=True)
    ap.add_argument("--path", default="")
    ap.add_argument("--rate", type=int, default=50)
    ap.add_argument("--seconds", type=int, default=30)
    a = ap.parse_args()
    direct = await run(a.pdns, a.pdns_key, a.path, a.rate, a.seconds)
    proxied = await run(a.soabox, a.soabox_key, a.path, a.rate, a.seconds)
    print(f"path {a.path or '/'} at {a.rate}/s for {a.seconds}s")
    print(f"direct  p50 {statistics.median(direct):7.2f} ms  p95 {p95(direct):7.2f} ms")
    print(
        f"soabox  p50 {statistics.median(proxied):7.2f} ms  p95 {p95(proxied):7.2f} ms"
    )
    print(f"added   p95 {p95(proxied) - p95(direct):7.2f} ms")


if __name__ == "__main__":
    asyncio.run(main())
