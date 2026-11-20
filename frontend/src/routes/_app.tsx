import { useQuery } from "@tanstack/react-query";
import { createFileRoute, Outlet, redirect } from "@tanstack/react-router";
import { isApiError } from "../api/client";
import { meQuery } from "../api/hooks/me";
import { AppShell } from "../components/app-shell";
import { LoadingState } from "../components/ui";
import { MeProvider } from "../lib/me-context";

export const Route = createFileRoute("/_app")({
  beforeLoad: async ({ context, location }) => {
    try {
      await context.queryClient.ensureQueryData(meQuery);
    } catch (error) {
      if (isApiError(error, 401)) {
        throw redirect({ to: "/login", search: { redirect: location.href } });
      }
      throw error;
    }
  },
  component: SignedInLayout,
});

function SignedInLayout() {
  const { data: me } = useQuery(meQuery);
  if (!me) {
    return <LoadingState />;
  }
  return (
    <MeProvider me={me}>
      <AppShell>
        <Outlet />
      </AppShell>
    </MeProvider>
  );
}
