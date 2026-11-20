import { useNavigate, useRouterState } from "@tanstack/react-router";
import {
  Boxes,
  ChevronDown,
  FileStack,
  Globe,
  History,
  KeyRound,
  LayoutDashboard,
  LogOut,
  Menu as MenuIcon,
  Monitor,
  Moon,
  Server,
  Settings,
  Shield,
  Sun,
  User,
  Users,
} from "lucide-react";
import { type ReactNode, useState } from "react";
import { useLogout } from "../api/hooks/me";
import type { Permission } from "../api/types";
import { useMe } from "../lib/me-context";
import { can } from "../lib/permissions";
import { readThemeChoice, saveThemeChoice, type ThemeChoice } from "../lib/theme";
import { Button, Drawer, Menu, MenuItem, MenuSection, MenuSeparator, MenuTrigger, NavLink } from "./ui";
import { ZoneQuickSearch } from "./zone-quick-search";

interface NavItem {
  href: string;
  label: string;
  icon: ReactNode;
  permission?: Permission;
}

const mainNav: NavItem[] = [
  { href: "/", label: "Dashboard", icon: <LayoutDashboard aria-hidden /> },
  { href: "/zones", label: "Zones", icon: <Globe aria-hidden /> },
  { href: "/history", label: "History", icon: <History aria-hidden />, permission: "view_history" },
  { href: "/templates", label: "Templates", icon: <FileStack aria-hidden /> },
  { href: "/api-keys", label: "API keys", icon: <KeyRound aria-hidden />, permission: "create_api_keys" },
  { href: "/server", label: "Server", icon: <Server aria-hidden />, permission: "view_server" },
];

const adminNav: NavItem[] = [
  { href: "/admin/users", label: "Users", icon: <Users aria-hidden />, permission: "manage_users" },
  { href: "/admin/accounts", label: "Accounts", icon: <Boxes aria-hidden />, permission: "manage_users" },
  { href: "/admin/settings", label: "Settings", icon: <Settings aria-hidden />, permission: "manage_settings" },
  { href: "/admin/tsig", label: "TSIG keys", icon: <Shield aria-hidden />, permission: "manage_tsig" },
];

function isCurrent(pathname: string, href: string): boolean {
  if (href === "/") {
    return pathname === "/";
  }
  return pathname === href || pathname.startsWith(`${href}/`);
}

function SidebarNav({ onNavigate }: { onNavigate?: () => void }) {
  const me = useMe();
  const pathname = useRouterState({ select: (s) => s.location.pathname });
  const visible = (items: NavItem[]) => items.filter((item) => !item.permission || can(me, item.permission));
  const admin = visible(adminNav);
  const link = (item: NavItem) => (
    <li key={item.href}>
      <NavLink
        href={item.href}
        isCurrent={isCurrent(pathname, item.href)}
        {...(onNavigate ? { onPress: onNavigate } : {})}
      >
        {item.icon}
        {item.label}
      </NavLink>
    </li>
  );
  return (
    <nav aria-label="Main" className="flex flex-1 flex-col gap-5 overflow-y-auto px-3 py-4">
      <ul className="flex flex-col gap-0.5">{visible(mainNav).map(link)}</ul>
      {admin.length > 0 && (
        <div className="flex flex-col gap-1">
          <h2 className="px-2.5 text-xs font-medium tracking-wide text-fg-muted uppercase">Administration</h2>
          <ul className="flex flex-col gap-0.5">{admin.map(link)}</ul>
        </div>
      )}
    </nav>
  );
}

function Brand() {
  return (
    <div className="flex h-14 shrink-0 items-center gap-2 border-b border-border px-5 text-base font-semibold text-fg">
      <span aria-hidden className="flex size-6 items-center justify-center rounded bg-accent text-accent-fg">
        <Globe className="size-4" />
      </span>
      SOAbox
    </div>
  );
}

const themeOptions: { id: ThemeChoice; label: string; icon: ReactNode }[] = [
  { id: "system", label: "System theme", icon: <Monitor aria-hidden /> },
  { id: "light", label: "Light theme", icon: <Sun aria-hidden /> },
  { id: "dark", label: "Dark theme", icon: <Moon aria-hidden /> },
];

function UserMenu() {
  const me = useMe();
  const navigate = useNavigate();
  const logout = useLogout();
  const [theme, setTheme] = useState<ThemeChoice>(readThemeChoice);

  const onAction = (key: string) => {
    if (key === "logout") {
      logout.mutate(undefined, {
        onSettled: () => {
          void navigate({ to: "/login", search: {} });
        },
      });
      return;
    }
    const choice = themeOptions.find((option) => `theme-${option.id}` === key);
    if (choice) {
      saveThemeChoice(choice.id);
      setTheme(choice.id);
    }
  };

  return (
    <MenuTrigger>
      <Button variant="ghost" aria-label={`Account menu for ${me.user.display_name || me.user.username}`}>
        <User aria-hidden />
        <span className="hidden max-w-40 truncate sm:inline">{me.user.display_name || me.user.username}</span>
        <ChevronDown aria-hidden />
      </Button>
      <Menu label="Account" onAction={onAction}>
        <MenuItem id="profile" href="/profile">
          <User aria-hidden />
          Profile
        </MenuItem>
        <MenuSeparator />
        <MenuSection title="Appearance">
          {themeOptions.map((option) => (
            <MenuItem key={option.id} id={`theme-${option.id}`} textValue={option.label}>
              {option.icon}
              <span className="flex-1">{option.label}</span>
              {theme === option.id && <span className="text-xs text-accent-text">Active</span>}
            </MenuItem>
          ))}
        </MenuSection>
        <MenuSeparator />
        <MenuItem id="logout">
          <LogOut aria-hidden />
          Sign out
        </MenuItem>
      </Menu>
    </MenuTrigger>
  );
}

/** The signed-in layout: sidebar, top bar, and main content. */
export function AppShell({ children }: { children: ReactNode }) {
  const [drawerOpen, setDrawerOpen] = useState(false);
  return (
    <div className="min-h-screen md:pl-60">
      <a
        href="#main"
        className="sr-only z-50 rounded bg-surface px-3 py-2 text-sm focus:not-sr-only focus:fixed focus:top-2 focus:left-2"
      >
        Skip to content
      </a>
      <aside className="fixed inset-y-0 left-0 hidden w-60 flex-col border-r border-border bg-surface md:flex">
        <Brand />
        <SidebarNav />
      </aside>
      <Drawer isOpen={drawerOpen} onOpenChange={setDrawerOpen} label="Navigation">
        <Brand />
        <SidebarNav onNavigate={() => setDrawerOpen(false)} />
      </Drawer>
      <header className="sticky top-0 z-30 flex h-14 items-center gap-3 border-b border-border bg-surface px-4">
        <span className="md:hidden">
          <Button variant="ghost" size="icon" aria-label="Open navigation" onPress={() => setDrawerOpen(true)}>
            <MenuIcon aria-hidden />
          </Button>
        </span>
        <ZoneQuickSearch />
        <div className="ml-auto">
          <UserMenu />
        </div>
      </header>
      <main id="main" className="mx-auto max-w-[1400px] p-4 md:p-6">
        {children}
      </main>
    </div>
  );
}
