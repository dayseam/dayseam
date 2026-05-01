import { createContext } from "react";

export type DistributionProfileLoaded = "direct" | "mas";

export type DistributionProfileState = "loading" | DistributionProfileLoaded;

export const DistributionProfileContext =
  createContext<DistributionProfileState>("loading");
