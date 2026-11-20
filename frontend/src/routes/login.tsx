import { zodResolver } from "@hookform/resolvers/zod";
import { createFileRoute, redirect, useNavigate } from "@tanstack/react-router";
import { History, KeyRound, LogIn, ShieldCheck } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { Controller, useForm } from "react-hook-form";
import { z } from "zod";
import { errorMessage, isApiError } from "../api/client";
import { meKeys, useAuthConfig, useLogin } from "../api/hooks/me";
import { Logo } from "../components/logo";
import { Alert, Button, DocumentLink, Form, LoadingState, TextField } from "../components/ui";
import { APP_VERSION } from "../lib/version";

const searchSchema = z.object({
  redirect: z.string().optional().catch(undefined),
  error: z.string().optional().catch(undefined),
});

export const Route = createFileRoute("/login")({
  validateSearch: searchSchema,
  beforeLoad: ({ context }) => {
    if (context.queryClient.getQueryData(meKeys.me)) {
      throw redirect({ to: "/" });
    }
  },
  component: LoginPage,
});

const SIGN_IN_ERRORS: Record<string, string> = {
  pending_approval: "Your account is waiting for an administrator to approve it.",
  disabled: "Your account is disabled. Contact an administrator.",
  oidc_failed: "Single sign-on did not complete. Please try again.",
};

const credentialsSchema = z.object({
  username: z.string().trim().min(1, "Enter your username."),
  password: z.string().min(1, "Enter your password."),
});
type Credentials = z.infer<typeof credentialsSchema>;

const totpSchema = z.object({
  code: z
    .string()
    .trim()
    .regex(/^\d{6,8}$/, "Enter the code from your authenticator app."),
});
type TotpForm = z.infer<typeof totpSchema>;

/** Only follow same-site relative redirects. */
function safeRedirect(target: string | undefined): string {
  if (!target?.startsWith("/") || target.startsWith("//") || target.includes("\\") || target.startsWith("/login")) {
    return "/";
  }
  return target;
}

function LoginPage() {
  const search = Route.useSearch();
  const navigate = useNavigate();
  const config = useAuthConfig();
  const login = useLogin();
  const [credentials, setCredentials] = useState<Credentials | null>(null);
  const [error, setError] = useState<string | null>(search.error ? (SIGN_IN_ERRORS[search.error] ?? null) : null);
  const headingRef = useRef<HTMLHeadingElement>(null);

  useEffect(() => {
    headingRef.current?.focus({ preventScroll: true });
  }, []);

  const credentialsForm = useForm<Credentials>({
    resolver: zodResolver(credentialsSchema),
    defaultValues: { username: "", password: "" },
  });
  const totpForm = useForm<TotpForm>({ resolver: zodResolver(totpSchema), defaultValues: { code: "" } });

  const finish = () => {
    void navigate({ href: safeRedirect(search.redirect), replace: true });
  };

  const submitCredentials = credentialsForm.handleSubmit((values) => {
    setError(null);
    login.mutate(values, {
      onSuccess: finish,
      onError: (err) => {
        if (isApiError(err, 401) && err.code === "TOTP_REQUIRED") {
          setCredentials(values);
          return;
        }
        setError(errorMessage(err));
      },
    });
  });

  const submitTotp = totpForm.handleSubmit((values) => {
    if (!credentials) {
      return;
    }
    setError(null);
    login.mutate(
      { ...credentials, totp_code: values.code },
      {
        onSuccess: finish,
        onError: (err) => {
          setError(errorMessage(err));
          totpForm.setFocus("code");
        },
      },
    );
  });

  return (
    <main className="grid min-h-screen bg-bg lg:grid-cols-[minmax(0,1fr)_minmax(0,1.1fr)]">
      <section
        aria-label="About SOAbox"
        className="relative hidden flex-col justify-between overflow-hidden bg-sidebar p-12 text-sidebar-fg lg:flex"
      >
        <div
          aria-hidden
          className="pointer-events-none absolute inset-0 opacity-[0.07] [background-image:linear-gradient(var(--sb-sidebar-fg)_1px,transparent_1px),linear-gradient(90deg,var(--sb-sidebar-fg)_1px,transparent_1px)] [background-size:32px_32px]"
        />
        <div
          aria-hidden
          className="pointer-events-none absolute -top-40 -left-40 size-[520px] rounded-full bg-blue-600/25 blur-3xl"
        />
        <div className="relative text-white">
          <Logo size={32} />
        </div>
        <div className="relative flex max-w-md flex-col gap-8">
          <div className="flex flex-col gap-3">
            <p className="text-3xl leading-tight font-semibold tracking-tight text-white">
              Authoritative DNS, managed with confidence.
            </p>
            <p className="text-sidebar-muted">
              A web console and drop-in PowerDNS API for teams that run their own zones.
            </p>
          </div>
          <ul className="flex flex-col gap-4 text-sm">
            {[
              { icon: KeyRound, title: "PowerDNS compatible API", text: "Existing tools and scripts keep working." },
              { icon: ShieldCheck, title: "Per-zone access control", text: "Roles, accounts, and scoped API keys." },
              { icon: History, title: "Complete audit history", text: "Every change, by whom, with a diff." },
            ].map((item) => (
              <li key={item.title} className="flex gap-3">
                <span className="flex size-8 shrink-0 items-center justify-center rounded-lg bg-white/5 ring-1 ring-white/10">
                  <item.icon aria-hidden className="size-4 text-sidebar-accent" />
                </span>
                <span>
                  <span className="block font-medium text-white">{item.title}</span>
                  <span className="text-sidebar-muted">{item.text}</span>
                </span>
              </li>
            ))}
          </ul>
        </div>
        <p className="relative text-xs text-sidebar-muted">SOAbox {APP_VERSION}. Open source under Apache-2.0.</p>
      </section>
      <div className="flex items-center justify-center p-6">
        <div className="w-full max-w-sm">
          <div className="mb-8 lg:hidden">
            <Logo size={32} />
          </div>
          <h1
            ref={headingRef}
            tabIndex={-1}
            className="mb-1 text-2xl font-semibold tracking-tight text-fg outline-none"
          >
            Sign in to SOAbox
          </h1>
          <p className="mb-6 text-sm text-fg-muted">
            {credentials
              ? "Enter the code from your authenticator app."
              : config.data?.oidc_enabled
                ? "Use single sign-on or your SOAbox account."
                : "Use your SOAbox account."}
          </p>

          {error && (
            <div className="mb-4">
              <Alert tone="danger">{error}</Alert>
            </div>
          )}

          {config.isPending ? (
            <LoadingState />
          ) : credentials ? (
            <Form onSubmit={submitTotp} label="Authenticator code">
              <Controller
                control={totpForm.control}
                name="code"
                render={({ field, fieldState }) => (
                  <TextField
                    label="Authentication code"
                    value={field.value}
                    onChange={field.onChange}
                    onBlur={field.onBlur}
                    inputRef={field.ref}
                    errorMessage={fieldState.error?.message}
                    autoComplete="one-time-code"
                    inputMode="numeric"
                    autoFocus
                    mono
                  />
                )}
              />
              <Button type="submit" variant="primary" isPending={login.isPending}>
                Verify
              </Button>
              <Button
                variant="ghost"
                onPress={() => {
                  setCredentials(null);
                  totpForm.reset();
                }}
              >
                Back
              </Button>
            </Form>
          ) : (
            <div className="flex flex-col gap-4">
              {config.data?.local_enabled !== false && (
                <Form onSubmit={submitCredentials} label="Sign in">
                  <Controller
                    control={credentialsForm.control}
                    name="username"
                    render={({ field, fieldState }) => (
                      <TextField
                        label="Username"
                        value={field.value}
                        onChange={field.onChange}
                        onBlur={field.onBlur}
                        inputRef={field.ref}
                        errorMessage={fieldState.error?.message}
                        autoComplete="username"
                      />
                    )}
                  />
                  <Controller
                    control={credentialsForm.control}
                    name="password"
                    render={({ field, fieldState }) => (
                      <TextField
                        label="Password"
                        type="password"
                        value={field.value}
                        onChange={field.onChange}
                        onBlur={field.onBlur}
                        inputRef={field.ref}
                        errorMessage={fieldState.error?.message}
                        autoComplete="current-password"
                      />
                    )}
                  />
                  <Button type="submit" variant="primary" isPending={login.isPending}>
                    <LogIn aria-hidden />
                    Sign in
                  </Button>
                </Form>
              )}
              {config.data?.oidc_enabled && (
                <>
                  {config.data.local_enabled && (
                    <div className="flex items-center gap-3 text-xs text-fg-muted">
                      <span className="h-px flex-1 bg-border" />
                      or
                      <span className="h-px flex-1 bg-border" />
                    </div>
                  )}
                  <DocumentLink href="/auth/oidc/start" variant="secondary">
                    <span className="w-full text-center">{config.data.oidc_label || "Single sign-on"}</span>
                  </DocumentLink>
                </>
              )}
              {config.isError && <Alert tone="danger">{errorMessage(config.error)}</Alert>}
            </div>
          )}
        </div>
      </div>
    </main>
  );
}
