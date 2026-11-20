#!/usr/bin/env node
// Fails when the hand-written API types in src/api/types.ts drift from the
// backend OpenAPI document. Each mapped schema must have exactly the same
// field names as its TypeScript interface (inherited fields included).
//
// Usage: node scripts/check-api-types.mjs ../backend/openapi.json

import { readFileSync } from "node:fs";

const MAP = {
  AuthConfig: "AuthConfig",
  LoginRequest: "LoginRequest",
  ErrorBody: "ErrorBody",
  Me: "Me",
  MeUser: "MeUser",
  Permissions: "Permissions",
  ProfileUpdate: "UpdateMeRequest",
  PasswordChange: "ChangePasswordRequest",
  TotpSetup: "TotpSetup",
  HistoryEntry: "HistoryEntry",
  HistoryPage: "HistoryPage",
  Dashboard: "Dashboard",
  AccountRef: "AccountRef",
  ZoneSummary: "ZoneSummary",
  ZonePage: "ZoneList",
  CreateZone: "CreateZoneRequest",
  UpdateZone: "UpdateZoneRequest",
  RrSetChange: "RrSetChange",
  RrSetPatch: "PatchRrSetsRequest",
  ActionResult: "ActionResult",
  ZoneExport: "ZoneExport",
  DnssecToggle: "SetDnssecRequest",
  NewCryptokey: "CreateCryptoKeyRequest",
  CryptokeyUpdate: "UpdateCryptoKeyRequest",
  TemplateRecord: "TemplateRecord",
  Template: "Template",
  CreateUser: "CreateUserRequest",
  UpdateUser: "UpdateUserRequest",
  AccountView: "Account",
  CreatedKey: "CreatedApiKey",
  KeyInput: "ApiKeyInput",
  Settings: "Settings",
  OidcSettings: "OidcSettings",
  OidcSettingsUpdate: "OidcSettingsUpdate",
  NewTsig: "CreateTsigKeyRequest",
};

const specPath = process.argv[2] ?? "../backend/openapi.json";
const spec = JSON.parse(readFileSync(specPath, "utf8"));
const source = readFileSync(new URL("../src/api/types.ts", import.meta.url), "utf8");

function interfaceFields(name, seen = new Set()) {
  if (seen.has(name)) return new Set();
  seen.add(name);
  const re = new RegExp(`export interface ${name}(?:\\s+extends\\s+([\\w,\\s]+))?\\s*\\{([\\s\\S]*?)\\n\\}`);
  const m = source.match(re);
  if (!m) return null;
  const fields = new Set();
  let depth = 0;
  for (const line of m[2].split("\n")) {
    const field = depth === 0 ? line.match(/^\s{2}(?:readonly\s+)?(\w+)\??:/) : null;
    if (field) fields.add(field[1]);
    depth += (line.match(/[{(]/g) ?? []).length - (line.match(/[})]/g) ?? []).length;
  }
  for (const parent of (m[1] ?? "")
    .split(",")
    .map((s) => s.trim())
    .filter(Boolean)) {
    for (const f of interfaceFields(parent, seen) ?? []) fields.add(f);
  }
  return fields;
}

let failures = 0;
for (const [schemaName, tsName] of Object.entries(MAP)) {
  const schema = spec.components?.schemas?.[schemaName];
  if (!schema) {
    console.error(`missing schema ${schemaName} in ${specPath}`);
    failures++;
    continue;
  }
  const schemaFields = new Set(Object.keys(schema.properties ?? {}));
  const tsFields = interfaceFields(tsName);
  if (!tsFields) {
    console.error(`missing interface ${tsName} in src/api/types.ts`);
    failures++;
    continue;
  }
  const missing = [...schemaFields].filter((f) => !tsFields.has(f));
  const extra = [...tsFields].filter((f) => !schemaFields.has(f));
  if (missing.length || extra.length) {
    failures++;
    console.error(`${schemaName} -> ${tsName}: missing [${missing.join(", ")}] extra [${extra.join(", ")}]`);
  }
}

if (failures) {
  console.error(`${failures} API type mismatches`);
  process.exit(1);
}
console.log(`${Object.keys(MAP).length} API types match ${specPath}`);
