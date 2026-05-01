import { useContext } from "react";
import {
  DistributionProfileContext,
  type DistributionProfileState,
} from "./distributionProfileContext";

export function useDistributionProfile(): DistributionProfileState {
  return useContext(DistributionProfileContext);
}
