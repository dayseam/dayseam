import { useEffect, useState, type ReactNode } from "react";
import { invoke } from "../ipc/invoke";
import {
  DistributionProfileContext,
  type DistributionProfileState,
} from "./distributionProfileContext";

/** Resolves `distribution_profile` once so hooks can gate SKU-specific UX. */
export function DistributionProfileProvider({
  children,
}: {
  children: ReactNode;
}) {
  const [profile, setProfile] = useState<DistributionProfileState>("loading");
  useEffect(() => {
    let cancelled = false;
    void invoke("distribution_profile", {})
      .then((p) => {
        if (cancelled) return;
        setProfile(p === "mas" ? "mas" : "direct");
      })
      .catch(() => {
        if (!cancelled) setProfile("direct");
      });
    return () => {
      cancelled = true;
    };
  }, []);
  return (
    <DistributionProfileContext.Provider value={profile}>
      {children}
    </DistributionProfileContext.Provider>
  );
}
