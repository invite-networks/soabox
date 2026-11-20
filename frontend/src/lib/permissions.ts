import type { Me, Permission } from "../api/types";

/** Returns true when the current user holds a permission. */
export function can(me: Me | undefined, permission: Permission): boolean {
  return me?.permissions[permission] === true;
}
