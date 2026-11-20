import type { ReactNode } from "react";
import type { Permission } from "../api/types";
import { useMe } from "../lib/me-context";
import { can } from "../lib/permissions";
import { Alert, PageHeader } from "./ui";

/** Renders children only when the user holds the permission. Otherwise shows a message in place. */
export function RequirePermission({
  permission,
  title,
  children,
}: {
  permission: Permission;
  title: string;
  children: ReactNode;
}) {
  const me = useMe();
  if (!can(me, permission)) {
    return (
      <>
        <PageHeader title={title} />
        <Alert tone="warning" title="You do not have permission to view this page">
          Ask an administrator if you need access.
        </Alert>
      </>
    );
  }
  return <>{children}</>;
}
