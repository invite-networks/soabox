/**
 * Types for the SOAbox UI API (`/v1` and `/auth`).
 *
 * Hand-written from docs/spec/v1-api.md until the backend publishes openapi.json.
 * Times are Unix seconds.
 */

export type UnixSeconds = number;

export const ERROR_CODES = [
  "BAD_REQUEST",
  "VALIDATION_ERROR",
  "UNAUTHENTICATED",
  "TOTP_REQUIRED",
  "FORBIDDEN",
  "NOT_FOUND",
  "CONFLICT",
  "RATE_LIMITED",
  "UPSTREAM_ERROR",
  "UPSTREAM_UNAVAILABLE",
  "INTERNAL_ERROR",
  "CSRF_FAILED",
] as const;
export type ErrorCode = (typeof ERROR_CODES)[number];

export interface ErrorBody {
  detail: string;
  code: string;
  status: number;
}

// Sign-in

export interface AuthConfig {
  local_enabled: boolean;
  oidc_enabled: boolean;
  oidc_label: string;
}

export interface LoginRequest {
  username: string;
  password: string;
  totp_code?: string;
}

// Current user

export type Role = "admin" | "operator" | "user";
export type AuthSource = "local" | "oidc";
export type UserStatus = "active" | "pending" | "disabled";

export interface Permissions {
  manage_users: boolean;
  manage_settings: boolean;
  manage_tsig: boolean;
  view_server: boolean;
  all_zones: boolean;
  create_zones: boolean;
  delete_zones: boolean;
  view_history: boolean;
  manage_dnssec: boolean;
  manage_templates: boolean;
  create_api_keys: boolean;
}

export type Permission = keyof Permissions;

export interface MeUser {
  id: number;
  username: string;
  display_name: string;
  email: string;
  role: Role;
  auth_source: AuthSource;
  totp_enabled: boolean;
}

export interface Me {
  user: MeUser;
  csrf_token: string;
  permissions: Permissions;
}

export interface UpdateMeRequest {
  display_name: string;
  email: string;
}

export interface ChangePasswordRequest {
  current_password: string;
  new_password: string;
}

export interface TotpSetup {
  secret: string;
  otpauth_url: string;
}

// History

export interface RrSetSnapshot {
  ttl: number;
  records: RecordItem[];
}

/** One changed RRset. `before` null means added, `after` null means removed. */
export interface RecordChange {
  name: string;
  type: string;
  before: RrSetSnapshot | null;
  after: RrSetSnapshot | null;
}

/** One changed zone setting, from `zone.update`. */
export interface ZoneFieldChange {
  field: string;
  before: unknown;
  after: unknown;
}

export type HistoryDiff = RecordChange[] | { zone: ZoneFieldChange[] };

export interface HistoryEntry {
  id: number;
  at: UnixSeconds;
  actor_type: string;
  actor_id: number | null;
  actor_name: string;
  client_ip: string | null;
  operation: string;
  zone_name: string | null;
  /** PowerDNS URL id of the zone, for links. */
  zone_id: string | null;
  outcome: "success" | "failure";
  upstream_status: number | null;
  diff: HistoryDiff | null;
  /** The redacted request for operations without a diff, or `{request, error}` for failures. */
  detail: unknown;
}

export interface HistoryPage {
  items: HistoryEntry[];
}

export interface HistoryFilters {
  zone?: string;
  actor?: string;
  operation?: string;
  since?: UnixSeconds;
  until?: UnixSeconds;
  before_id?: number;
  limit?: number;
}

// Dashboard and server

export interface Dashboard {
  zone_count: number;
  recent_history: HistoryEntry[];
  /** `uptime` is a string of seconds as PowerDNS reports it. */
  server: { version: string; uptime: string | null } | null;
}

export interface ServerInfo {
  id: string;
  daemon_type: string;
  version: string;
}

export interface StatisticMapEntry {
  name: string;
  value: string;
}

export interface ServerItem {
  name: string;
  type: string;
  value: string | StatisticMapEntry[];
}

// Zones

export type ZoneKind = "Native" | "Master" | "Slave";

export interface AccountRef {
  id: number;
  name: string;
}

export interface ZoneSummary {
  id: string;
  name: string;
  kind: ZoneKind;
  serial: number;
  dnssec: boolean;
  account: AccountRef | null;
  masters: string[];
}

export interface RecordItem {
  content: string;
  disabled: boolean;
}

export interface RecordComment {
  content: string;
  account: string;
  modified_at: UnixSeconds;
}

export interface RrSet {
  name: string;
  type: string;
  ttl: number;
  records: RecordItem[];
  comments: RecordComment[];
}

export interface Zone extends ZoneSummary {
  soa_edit_api: string;
  api_rectify: boolean;
  nsec3param: string;
  /** Record types the backend accepts in this zone, from the forward or reverse list in settings. */
  allowed_record_types: string[];
  rrsets: RrSet[];
}

export interface ZoneList {
  items: ZoneSummary[];
  total: number;
}

export interface ZoneListParams {
  q?: string;
  page?: number;
  per_page?: number;
}

export interface CreateZoneRequest {
  name: string;
  kind: ZoneKind;
  masters: string[];
  nameservers: string[];
  template_id?: number;
  account_id?: number;
  soa_edit_api?: string;
}

export interface UpdateZoneRequest {
  kind?: ZoneKind;
  masters?: string[];
  account_id?: number | null;
  soa_edit_api?: string;
  api_rectify?: boolean;
}

export type ChangeType = "REPLACE" | "DELETE";

export interface RrSetChange {
  name: string;
  type: string;
  ttl: number;
  changetype: ChangeType;
  records: RecordItem[];
  comments?: RecordComment[];
}

export interface PatchRrSetsRequest {
  rrsets: RrSetChange[];
}

export interface ActionResult {
  result: string;
}

export interface ZoneExport {
  zone: string;
}

// DNSSEC and metadata

export type KeyType = "ksk" | "zsk" | "csk";

export interface CryptoKey {
  id: number;
  keytype: KeyType;
  active: boolean;
  published: boolean;
  algorithm: string;
  bits: number;
  dnskey: string;
  ds: string[];
}

export interface SetDnssecRequest {
  enabled: boolean;
  nsec3param?: string;
}

export interface CreateCryptoKeyRequest {
  keytype: KeyType;
  active: boolean;
  algorithm?: string;
  bits?: number;
}

export interface UpdateCryptoKeyRequest {
  active: boolean;
  published: boolean;
}

export interface Metadata {
  kind: string;
  metadata: string[];
}

// Templates

export interface TemplateRecord {
  name: string;
  type: string;
  ttl: number;
  content: string;
}

export interface Template {
  id: number;
  name: string;
  description: string;
  records: TemplateRecord[];
}

export type TemplateInput = Omit<Template, "id">;

// Users and accounts

export interface User {
  id: number;
  username: string;
  display_name: string;
  email: string;
  role: Role;
  auth_source: AuthSource;
  status: UserStatus;
  totp_enabled: boolean;
  created_at: UnixSeconds;
  last_login_at: UnixSeconds | null;
  zones: string[];
  account_ids: number[];
}

export interface CreateUserRequest {
  username: string;
  display_name: string;
  email: string;
  role: Role;
  password: string;
}

export interface UpdateUserRequest {
  display_name: string;
  email: string;
  role: Role;
  status: UserStatus;
  zones: string[];
  account_ids: number[];
}

export interface Account {
  id: number;
  name: string;
  description: string;
  zones: string[];
  user_ids: number[];
}

export interface CreateAccountRequest {
  name: string;
  description: string;
}

export interface UpdateAccountRequest {
  name: string;
  description: string;
  zones: string[];
  user_ids: number[];
}

// API keys

export type ApiKeyRole = "admin" | "operator" | "scoped";

export interface ApiKey {
  id: number;
  hint: string;
  description: string;
  role: ApiKeyRole;
  zones: string[];
  account_ids: number[];
  create_account_id: number | null;
  networks: string[];
  expires_at: UnixSeconds | null;
  last_used_at: UnixSeconds | null;
  created_at: UnixSeconds;
  disabled: boolean;
  owner_user_id: number | null;
}

export interface ApiKeyInput {
  description: string;
  role: ApiKeyRole;
  zones: string[];
  account_ids: number[];
  create_account_id: number | null;
  networks: string[];
  expires_at: UnixSeconds | null;
}

export interface CreatedApiKey {
  key: ApiKey;
  secret: string;
}

// Settings

export interface Settings {
  local_login_with_oidc: "allow" | "deny";
  oidc_new_user: "create" | "approve";
  user_can_create_zones: boolean;
  user_can_delete_zones: boolean;
  user_can_view_history: boolean;
  user_can_manage_dnssec: boolean;
  user_can_create_api_keys: boolean;
  record_types_forward: string[];
  record_types_reverse: string[];
  auto_ptr: boolean;
}

/** Where the single sign-on settings in force come from. */
export type OidcSource = "environment" | "database";

/** Single sign-on provider settings. The client secret is never returned. */
export interface OidcSettings {
  source: OidcSource;
  enabled: boolean;
  issuer: string;
  client_id: string;
  client_secret_set: boolean;
  button_label: string;
  redirect_uri: string;
}

export interface OidcSettingsUpdate {
  enabled: boolean;
  issuer: string;
  client_id: string;
  /** A new secret. Omit to keep the stored one. */
  client_secret?: string;
  button_label: string;
}

// TSIG

export interface TsigKey {
  id: string;
  name: string;
  algorithm: string;
  key?: string;
}

export interface CreateTsigKeyRequest {
  name: string;
  algorithm: string;
  key?: string;
}
