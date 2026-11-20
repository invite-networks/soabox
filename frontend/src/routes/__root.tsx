import { createRootRouteWithContext, Outlet, useRouter } from "@tanstack/react-router";
import type { RouterContext } from "../app";
import { NotFound, RouteError } from "../components/route-states";
import { RouterBridge } from "../components/ui";

export const Route = createRootRouteWithContext<RouterContext>()({
  component: Root,
  errorComponent: RouteError,
  notFoundComponent: NotFound,
});

function Root() {
  const router = useRouter();
  return (
    <RouterBridge navigate={(href) => void router.navigate({ href })}>
      <Outlet />
    </RouterBridge>
  );
}
