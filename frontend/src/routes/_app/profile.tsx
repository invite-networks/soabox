import { zodResolver } from "@hookform/resolvers/zod";
import { createFileRoute } from "@tanstack/react-router";
import { useState } from "react";
import { Controller, useForm } from "react-hook-form";
import { z } from "zod";
import { errorMessage } from "../../api/client";
import { useChangePassword, useTotpDisable, useTotpEnable, useTotpSetup, useUpdateMe } from "../../api/hooks/me";
import {
  Alert,
  Badge,
  Button,
  Card,
  CodeBlock,
  CopyButton,
  DetailList,
  FieldGrid,
  Form,
  PageHeader,
  Stack,
  TextField,
} from "../../components/ui";
import { useMe } from "../../lib/me-context";

export const Route = createFileRoute("/_app/profile")({
  component: ProfilePage,
});

const MIN_PASSWORD = 12;

const detailsSchema = z.object({
  display_name: z.string().trim().max(200),
  email: z.union([z.literal(""), z.email("Enter a valid email address.")]),
});
type Details = z.infer<typeof detailsSchema>;

function DetailsForm() {
  const me = useMe();
  const update = useUpdateMe();
  const form = useForm<Details>({
    resolver: zodResolver(detailsSchema),
    defaultValues: { display_name: me.user.display_name, email: me.user.email },
  });
  return (
    <Form label="Your details" onSubmit={form.handleSubmit((values) => update.mutate(values))}>
      {update.isError && <Alert tone="danger">{errorMessage(update.error)}</Alert>}
      {update.isSuccess && <Alert tone="success">Your details were saved.</Alert>}
      <FieldGrid>
        <Controller
          control={form.control}
          name="display_name"
          render={({ field, fieldState }) => (
            <TextField
              label="Display name"
              value={field.value}
              onChange={field.onChange}
              inputRef={field.ref}
              errorMessage={fieldState.error?.message}
              autoComplete="name"
            />
          )}
        />
        <Controller
          control={form.control}
          name="email"
          render={({ field, fieldState }) => (
            <TextField
              label="Email"
              type="email"
              value={field.value}
              onChange={field.onChange}
              inputRef={field.ref}
              errorMessage={fieldState.error?.message}
              autoComplete="email"
            />
          )}
        />
      </FieldGrid>
      <div>
        <Button type="submit" variant="primary" isPending={update.isPending}>
          Save details
        </Button>
      </div>
    </Form>
  );
}

const passwordSchema = z
  .object({
    current_password: z.string().min(1, "Enter your current password."),
    new_password: z.string().min(MIN_PASSWORD, `Use at least ${MIN_PASSWORD} characters.`),
    confirm: z.string(),
  })
  .refine((v) => v.new_password === v.confirm, { path: ["confirm"], message: "The passwords do not match." });
type PasswordValues = z.infer<typeof passwordSchema>;

function PasswordForm() {
  const change = useChangePassword();
  const form = useForm<PasswordValues>({
    resolver: zodResolver(passwordSchema),
    defaultValues: { current_password: "", new_password: "", confirm: "" },
  });
  const field = (name: keyof PasswordValues, label: string, autoComplete: string) => (
    <Controller
      control={form.control}
      name={name}
      render={({ field: f, fieldState }) => (
        <TextField
          label={label}
          type="password"
          value={f.value}
          onChange={f.onChange}
          onBlur={f.onBlur}
          inputRef={f.ref}
          errorMessage={fieldState.error?.message}
          autoComplete={autoComplete}
        />
      )}
    />
  );
  return (
    <Form
      label="Change password"
      onSubmit={form.handleSubmit(({ current_password, new_password }) =>
        change.mutate({ current_password, new_password }, { onSuccess: () => form.reset() }),
      )}
    >
      {change.isError && <Alert tone="danger">{errorMessage(change.error)}</Alert>}
      {change.isSuccess && <Alert tone="success">Your password was changed.</Alert>}
      <FieldGrid columns={3}>
        {field("current_password", "Current password", "current-password")}
        {field("new_password", "New password", "new-password")}
        {field("confirm", "Confirm new password", "new-password")}
      </FieldGrid>
      <div>
        <Button type="submit" variant="primary" isPending={change.isPending}>
          Change password
        </Button>
      </div>
    </Form>
  );
}

const CODE = /^\d{6,8}$/;

function TotpSection() {
  const me = useMe();
  const setup = useTotpSetup();
  const enable = useTotpEnable();
  const disable = useTotpDisable();
  const [code, setCode] = useState("");
  const [codeError, setCodeError] = useState<string | undefined>();
  const [password, setPassword] = useState("");
  const [passwordError, setPasswordError] = useState<string | undefined>();

  const checkCode = () => {
    if (!CODE.test(code.trim())) {
      setCodeError("Enter the code from your authenticator app.");
      return false;
    }
    setCodeError(undefined);
    return true;
  };

  if (me.user.totp_enabled) {
    return (
      <Form
        label="Turn off two-factor authentication"
        onSubmit={() => {
          const passwordOk = password.length > 0;
          setPasswordError(passwordOk ? undefined : "Enter your current password.");
          if (checkCode() && passwordOk) {
            disable.mutate(
              { code: code.trim(), password },
              {
                onSuccess: () => {
                  setCode("");
                  setPassword("");
                },
              },
            );
          }
        }}
      >
        <p className="flex items-center gap-2 text-sm">
          <Badge tone="success">On</Badge> Sign-in asks for a code from your authenticator app.
        </p>
        {disable.isError && <Alert tone="danger">{errorMessage(disable.error)}</Alert>}
        <div className="flex max-w-xs flex-col gap-3">
          <TextField
            label="Current password"
            type="password"
            value={password}
            onChange={setPassword}
            errorMessage={passwordError}
            autoComplete="current-password"
          />
          <TextField
            label="Current code"
            value={code}
            onChange={setCode}
            errorMessage={codeError}
            inputMode="numeric"
            autoComplete="one-time-code"
            mono
          />
        </div>
        <div>
          <Button type="submit" variant="danger" isPending={disable.isPending}>
            Turn off two-factor
          </Button>
        </div>
      </Form>
    );
  }

  if (!setup.data) {
    return (
      <div className="flex flex-col gap-3">
        <p className="flex items-center gap-2 text-sm">
          <Badge>Off</Badge> Add a code from an authenticator app to your sign-in.
        </p>
        {setup.isError && <Alert tone="danger">{errorMessage(setup.error)}</Alert>}
        <div>
          <Button variant="primary" isPending={setup.isPending} onPress={() => setup.mutate()}>
            Set up two-factor
          </Button>
        </div>
      </div>
    );
  }

  return (
    <Form
      label="Confirm two-factor setup"
      onSubmit={() =>
        checkCode() &&
        enable.mutate(code.trim(), {
          onSuccess: () => {
            setCode("");
            setup.reset();
          },
        })
      }
    >
      <p className="text-sm">
        Add this account to your authenticator app with the secret or the setup link, then enter a code.
      </p>
      <DetailList
        items={[
          {
            label: "Secret",
            value: (
              <span className="flex flex-wrap items-center gap-2">
                <span className="font-mono">{setup.data.secret}</span>
                <CopyButton text={setup.data.secret} label="Copy secret" />
              </span>
            ),
          },
        ]}
      />
      <CodeBlock label="Setup link">{setup.data.otpauth_url}</CodeBlock>
      {enable.isError && <Alert tone="danger">{errorMessage(enable.error)}</Alert>}
      <div className="max-w-xs">
        <TextField
          label="Code from your app"
          value={code}
          onChange={setCode}
          errorMessage={codeError}
          inputMode="numeric"
          autoComplete="one-time-code"
          mono
        />
      </div>
      <div className="flex gap-2">
        <Button type="submit" variant="primary" isPending={enable.isPending}>
          Turn on two-factor
        </Button>
        <Button onPress={() => setup.reset()}>Cancel</Button>
      </div>
    </Form>
  );
}

function ProfilePage() {
  const me = useMe();
  const local = me.user.auth_source === "local";
  return (
    <>
      <PageHeader title="Profile" description={`Signed in as ${me.user.username}`} />
      <Stack>
        <Card title="Your details">
          <DetailsForm />
        </Card>
        {local && (
          <Card title="Password">
            <PasswordForm />
          </Card>
        )}
        {local && (
          <Card title="Two-factor authentication">
            <TotpSection />
          </Card>
        )}
      </Stack>
    </>
  );
}
