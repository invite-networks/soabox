import type { ReactNode } from "react";
import { RouterProvider } from "react-aria-components";

export interface RouterBridgeProps {
  navigate: (href: string) => void;
  children: ReactNode;
}

/** Routes React Aria links and menu items through the app router. */
export function RouterBridge({ navigate, children }: RouterBridgeProps) {
  return (
    <RouterProvider navigate={navigate} useHref={(href) => href}>
      {children}
    </RouterProvider>
  );
}
