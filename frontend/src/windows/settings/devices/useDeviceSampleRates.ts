import { useEffect, useState } from "react";
import { queryDeviceSampleRates, type RateQuery, type RateResult } from "./deviceSampleRates";

export function useDeviceSampleRates(
  { backend, device, outputDevice }: RateQuery,
  locked: boolean,
) {
  const [response, setResponse] = useState<{ key: string; result: RateResult }>();
  const key = JSON.stringify([backend, device, outputDevice]);
  useEffect(() => {
    if (locked || !backend || (backend === "asio" && !device)) {
      return undefined;
    }
    return queryDeviceSampleRates({ backend, device, outputDevice }, (result) => {
      setResponse({ key, result });
    });
  }, [backend, device, outputDevice, key, locked]);
  return response?.key === key ? response.result : undefined;
}
