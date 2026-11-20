import { type QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createBrowserHistory, createRouter, type RouterHistory, RouterProvider } from "@tanstack/react-router";
import { createQueryClient } from "./api/query-client";
import { NotFound, RouteError } from "./components/route-states";
import { routeTree } from "./routeTree.gen";

export interface RouterContext {
  queryClient: QueryClient;
}

function buildRouter(queryClient: QueryClient, history: RouterHistory) {
  return createRouter({
    routeTree,
    history,
    context: { queryClient },
    defaultPreload: "intent",
    defaultPreloadStaleTime: 0,
    defaultErrorComponent: RouteError,
    defaultNotFoundComponent: NotFound,
    scrollRestoration: true,
  });
}

export type AppRouter = ReturnType<typeof buildRouter>;

declare module "@tanstack/react-router" {
  interface Register {
    router: AppRouter;
  }
}

/** Creates the query client and router. Tests pass a memory history. */
export function createApp(history: RouterHistory = createBrowserHistory()) {
  let router: AppRouter | null = null;
  const queryClient = createQueryClient(() => {
    if (!router) {
      return;
    }
    const { pathname, href } = router.state.location;
    if (pathname === "/login") {
      return;
    }
    queryClient.removeQueries({ queryKey: ["me"] });
    void router.navigate({ to: "/login", search: { redirect: href } });
  });
  router = buildRouter(queryClient, history);
  return { router, queryClient };
}

export function App({ router, queryClient }: { router: AppRouter; queryClient: QueryClient }) {
  return (
    <QueryClientProvider client={queryClient}>
      <RouterProvider router={router} />
    </QueryClientProvider>
  );
}
