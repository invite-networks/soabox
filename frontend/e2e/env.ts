/** Reads the end-to-end credentials. Fails with a clear message when they are missing. */
export function credentials(): { username: string; password: string } {
  const username = process.env["SOABOX_E2E_USERNAME"];
  const password = process.env["SOABOX_E2E_PASSWORD"];
  if (!username || !password) {
    throw new Error("Set SOABOX_E2E_USERNAME and SOABOX_E2E_PASSWORD to a local Admin account.");
  }
  return { username, password };
}

/** A zone name that will not collide between runs. */
export function uniqueZone(prefix: string): string {
  return `${prefix}-${Date.now().toString(36)}.test.`;
}
