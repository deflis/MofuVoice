import { settingsApi } from "../api";
import type { DeviceSampleRates } from "../../../types";

export type RateQuery = { backend: string; device: string; outputDevice: string };
export type RateResult = { rates?: DeviceSampleRates; error?: string };

// Ignore results after a device change or after the settings screen closes.
export function queryDeviceSampleRates(
  query: RateQuery,
  receive: (result: RateResult) => void,
  getRates = settingsApi.deviceSampleRates,
) {
  let disposed = false;
  void getRates(query).then(
    (rates) => {
      if (!disposed) {
        receive({ rates });
      }
      return undefined;
    },
    (error: unknown) => {
      if (!disposed) {
        receive({ error: String(error) });
      }
      return undefined;
    },
  );
  return () => {
    disposed = true;
  };
}
