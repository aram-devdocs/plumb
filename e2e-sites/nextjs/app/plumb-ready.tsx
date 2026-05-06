"use client";

// Plumb e2e — readiness sentinel.
//
// The harness's `expected.json` carries a `wait_for.selector` of
// `html[data-plumb-ready="true"]`. Plumb's CDP driver polls for that
// selector after navigation and before snapshotting. Setting the
// attribute from a `useEffect` guarantees that React has hydrated the
// tree at least once on the client, which is the readiness signal we
// actually care about for layout-stable linting.
import { useEffect } from "react";

export default function PlumbReady() {
  useEffect(() => {
    document.documentElement.setAttribute("data-plumb-ready", "true");
  }, []);
  return null;
}
