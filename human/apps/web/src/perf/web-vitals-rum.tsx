"use client";

import { usePathname } from "next/navigation";
import { useReportWebVitals } from "next/web-vitals";
import { useCallback, useRef } from "react";

import {
  classifyPerformanceRoute,
  isWebVitalName,
  normalizeWebVital,
  type PerformanceRoute,
} from "./budgets";

type ReportWebVital = Parameters<typeof useReportWebVitals>[0];

function send(route: PerformanceRoute, metric: string, value: number): void {
  if (!isWebVitalName(metric)) {
    return;
  }
  const observed = normalizeWebVital(metric, value);
  if (observed === undefined) {
    return;
  }
  const body = JSON.stringify({ version: 1, route, metric, observed });
  if (navigator.sendBeacon("/api/performance/vitals", new Blob([body], { type: "application/json" }))) {
    return;
  }
  void fetch("/api/performance/vitals", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body,
    credentials: "same-origin",
    keepalive: true,
  }).catch(() => undefined);
}

export function WebVitalsRum() {
  const pathname = usePathname();
  const route = useRef(classifyPerformanceRoute(pathname));
  route.current = classifyPerformanceRoute(pathname);
  const report = useCallback<ReportWebVital>((metric) => {
    if (route.current !== undefined) {
      send(route.current, metric.name, metric.value);
    }
  }, []);
  useReportWebVitals(report);
  return null;
}
