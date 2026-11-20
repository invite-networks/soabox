import { MutationCache, QueryCache, QueryClient } from "@tanstack/react-query";
import { isApiError } from "./client";

/**
 * Creates the application QueryClient.
 *
 * @param onUnauthenticated Called when any query or mutation fails with `401`
 *   other than `TOTP_REQUIRED`, so the app can send the user to sign in.
 */
export function createQueryClient(onUnauthenticated: () => void = () => {}): QueryClient {
  const handle = (error: unknown) => {
    if (isApiError(error, 401) && error.code !== "TOTP_REQUIRED") {
      onUnauthenticated();
    }
  };
  return new QueryClient({
    queryCache: new QueryCache({ onError: handle }),
    mutationCache: new MutationCache({ onError: handle }),
    defaultOptions: {
      queries: {
        staleTime: 15_000,
        refetchOnWindowFocus: false,
        retry: (failureCount, error) => {
          if (isApiError(error) && error.status >= 400 && error.status < 500) {
            return false;
          }
          return failureCount < 1;
        },
      },
      mutations: { retry: false },
    },
  });
}
