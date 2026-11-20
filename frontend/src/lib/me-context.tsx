import { createContext, type ReactNode, useContext } from "react";
import type { Me } from "../api/types";

const MeContext = createContext<Me | null>(null);

export function MeProvider({ me, children }: { me: Me; children: ReactNode }) {
  return <MeContext.Provider value={me}>{children}</MeContext.Provider>;
}

/** The signed-in user. Only valid inside the signed-in layout. */
export function useMe(): Me {
  const me = useContext(MeContext);
  if (!me) {
    throw new Error("useMe must be used inside the signed-in layout.");
  }
  return me;
}
