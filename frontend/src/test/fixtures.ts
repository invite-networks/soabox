import type {
  Account,
  ApiKey,
  AuthConfig,
  CryptoKey,
  Dashboard,
  HistoryEntry,
  Me,
  Metadata,
  OidcSettings,
  Permissions,
  ServerInfo,
  ServerItem,
  Settings,
  Template,
  TsigKey,
  User,
  Zone,
  ZoneSummary,
} from "../api/types";

export const allPermissions: Permissions = {
  manage_users: true,
  manage_settings: true,
  manage_tsig: true,
  view_server: true,
  all_zones: true,
  create_zones: true,
  delete_zones: true,
  view_history: true,
  manage_dnssec: true,
  manage_templates: true,
  create_api_keys: true,
};

export const noPermissions: Permissions = {
  manage_users: false,
  manage_settings: false,
  manage_tsig: false,
  view_server: false,
  all_zones: false,
  create_zones: false,
  delete_zones: false,
  view_history: false,
  manage_dnssec: false,
  manage_templates: false,
  create_api_keys: false,
};

export const adminMe: Me = {
  user: {
    id: 1,
    username: "admin",
    display_name: "Admin",
    email: "admin@example.test",
    role: "admin",
    auth_source: "local",
    totp_enabled: false,
  },
  csrf_token: "csrf-test-token",
  permissions: allPermissions,
};

export const userMe: Me = {
  user: {
    id: 2,
    username: "alice",
    display_name: "Alice",
    email: "",
    role: "user",
    auth_source: "local",
    totp_enabled: false,
  },
  csrf_token: "csrf-user-token",
  permissions: { ...noPermissions, create_api_keys: true },
};

export const authConfig: AuthConfig = {
  local_enabled: true,
  oidc_enabled: true,
  oidc_label: "Sign in with Example ID",
};

export const zoneSummaries: ZoneSummary[] = [
  {
    id: "example.com.",
    name: "example.com.",
    kind: "Native",
    serial: 2026111901,
    dnssec: true,
    account: { id: 1, name: "Ops" },
    masters: [],
  },
  { id: "example.org.", name: "example.org.", kind: "Master", serial: 7, dnssec: false, account: null, masters: [] },
  {
    id: "2.0.192.in-addr.arpa.",
    name: "2.0.192.in-addr.arpa.",
    kind: "Native",
    serial: 1,
    dnssec: false,
    account: null,
    masters: [],
  },
];

export const exampleZone: Zone = {
  ...(zoneSummaries[0] as ZoneSummary),
  soa_edit_api: "DEFAULT",
  api_rectify: false,
  nsec3param: "",
  allowed_record_types: ["A", "AAAA", "CAA", "CNAME", "MX", "NS", "SRV", "TXT"],
  rrsets: [
    {
      name: "example.com.",
      type: "SOA",
      ttl: 3600,
      records: [
        { content: "ns1.example.com. hostmaster.example.com. 2026111901 10800 3600 604800 3600", disabled: false },
      ],
      comments: [],
    },
    {
      name: "example.com.",
      type: "NS",
      ttl: 3600,
      records: [
        { content: "ns1.example.com.", disabled: false },
        { content: "ns2.example.com.", disabled: false },
      ],
      comments: [],
    },
    {
      name: "www.example.com.",
      type: "A",
      ttl: 300,
      records: [{ content: "192.0.2.10", disabled: false }],
      comments: [],
    },
    {
      name: "old.example.com.",
      type: "A",
      ttl: 300,
      records: [{ content: "192.0.2.99", disabled: true }],
      comments: [],
    },
  ],
};

export const historyEntries: HistoryEntry[] = [
  {
    id: 42,
    at: 1_763_550_000,
    actor_type: "user",
    actor_id: 1,
    actor_name: "admin",
    client_ip: "198.51.100.7",
    operation: "rrsets.patch",
    zone_name: "example.com.",
    zone_id: "example.com.",
    outcome: "success",
    upstream_status: 204,
    diff: [
      {
        name: "www.example.com.",
        type: "A",
        before: { ttl: 300, records: [{ content: "192.0.2.1", disabled: false }] },
        after: { ttl: 300, records: [{ content: "192.0.2.10", disabled: false }] },
      },
    ],
    detail: null,
  },
  {
    id: 41,
    at: 1_763_540_000,
    actor_type: "api_key",
    actor_id: 1,
    actor_name: "deploy key",
    client_ip: null,
    operation: "zone.create",
    zone_name: "example.org.",
    zone_id: "example.org.",
    outcome: "failure",
    upstream_status: 422,
    diff: null,
    detail: { request: { name: "example.org.", kind: "Native" }, error: "Zone already exists" },
  },
  {
    id: 40,
    at: 1_763_530_000,
    actor_type: "user",
    actor_id: 1,
    actor_name: "admin",
    client_ip: "198.51.100.7",
    operation: "zone.update",
    zone_name: "0/26.2.0.192.in-addr.arpa.",
    zone_id: "0=2F26.2.0.192.in-addr.arpa.",
    outcome: "success",
    upstream_status: 204,
    diff: { zone: [{ field: "kind", before: "Native", after: "Master" }] },
    detail: null,
  },
];

export const dashboard: Dashboard = {
  zone_count: 3,
  recent_history: historyEntries,
  server: { version: "5.0.1", uptime: "93784" },
};

export const templates: Template[] = [
  {
    id: 1,
    name: "Basic web",
    description: "Apex and www",
    records: [
      { name: "@", type: "A", ttl: 3600, content: "192.0.2.1" },
      { name: "www", type: "CNAME", ttl: 3600, content: "example.com." },
    ],
  },
];

export const accounts: Account[] = [
  { id: 1, name: "Ops", description: "Operations team", zones: ["example.com."], user_ids: [2] },
  { id: 2, name: "Web", description: "", zones: [], user_ids: [] },
];

export const users: User[] = [
  {
    id: 1,
    username: "admin",
    display_name: "Admin",
    email: "admin@example.test",
    role: "admin",
    auth_source: "local",
    status: "active",
    totp_enabled: true,
    created_at: 1_763_000_000,
    last_login_at: 1_763_550_000,
    zones: [],
    account_ids: [],
  },
  {
    id: 3,
    username: "bob",
    display_name: "Bob",
    email: "",
    role: "user",
    auth_source: "oidc",
    status: "pending",
    totp_enabled: false,
    created_at: 1_763_100_000,
    last_login_at: null,
    zones: ["example.org."],
    account_ids: [1],
  },
];

export const apiKeys: ApiKey[] = [
  {
    id: 10,
    hint: "abcd",
    description: "CI deploy",
    role: "scoped",
    zones: ["example.com."],
    account_ids: [],
    create_account_id: null,
    networks: ["192.0.2.0/24"],
    expires_at: null,
    last_used_at: 1_763_500_000,
    created_at: 1_763_000_000,
    disabled: false,
    owner_user_id: 1,
  },
  {
    id: 11,
    hint: "wxyz",
    description: "Old automation",
    role: "admin",
    zones: [],
    account_ids: [],
    create_account_id: null,
    networks: [],
    expires_at: 1_700_000_000,
    last_used_at: null,
    created_at: 1_690_000_000,
    disabled: true,
    owner_user_id: 1,
  },
];

export const settings: Settings = {
  local_login_with_oidc: "allow",
  oidc_new_user: "approve",
  user_can_create_zones: false,
  user_can_delete_zones: false,
  user_can_view_history: true,
  user_can_manage_dnssec: false,
  user_can_create_api_keys: true,
  record_types_forward: ["A", "AAAA", "CNAME", "MX", "TXT", "SRV", "CAA", "NS"],
  record_types_reverse: ["PTR", "NS", "TXT"],
  auto_ptr: true,
};

export const oidcSettings: OidcSettings = {
  source: "database",
  enabled: false,
  issuer: "",
  client_id: "",
  client_secret_set: false,
  button_label: "Sign in with SSO",
  redirect_uri: "http://localhost:3000/auth/oidc/callback",
};

export const tsigKeys: TsigKey[] = [{ id: "transfer.", name: "transfer", algorithm: "hmac-sha256" }];

export const serverInfo: ServerInfo = { id: "localhost", daemon_type: "authoritative", version: "5.0.1" };

export const statistics: ServerItem[] = [
  { name: "uptime", type: "StatisticItem", value: "93784" },
  { name: "udp-queries", type: "StatisticItem", value: "1200" },
  { name: "response-by-qtype", type: "MapStatisticItem", value: [{ name: "A", value: "10" }] },
];

export const serverConfig: ServerItem[] = [
  { name: "api", type: "ConfigSetting", value: "yes" },
  { name: "local-port", type: "ConfigSetting", value: "53" },
];

export const cryptoKeys: CryptoKey[] = [
  {
    id: 1,
    keytype: "csk",
    active: true,
    published: true,
    algorithm: "ECDSAP256SHA256",
    bits: 256,
    dnskey: "257 3 13 AAAA",
    ds: ["example.com. IN DS 12345 13 2 ABCDEF"],
  },
];

export const metadata: Metadata[] = [{ kind: "ALLOW-AXFR-FROM", metadata: ["192.0.2.0/24"] }];
