import type { ErrorComponentProps } from "@tanstack/react-router";
import { ErrorState, Link, PageHeader } from "./ui";

/** Shown when a route fails to load or render. */
export function RouteError({ error, reset }: ErrorComponentProps) {
  return (
    <div className="p-4">
      <PageHeader title="Something went wrong" />
      <ErrorState error={error} onRetry={reset} title="This page could not be shown" />
    </div>
  );
}

/** Shown for unknown paths. */
export function NotFound() {
  return (
    <div className="p-4">
      <PageHeader title="Page not found" description="The page you asked for does not exist." />
      <Link href="/">Go to the dashboard</Link>
    </div>
  );
}
