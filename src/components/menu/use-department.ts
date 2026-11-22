"use client";

import { useCallback, useEffect, useState } from "react";

import { loadSettings, saveSettings } from "../settings/actions";
import { DEPT_EVENT } from "./config";

/**
 * The department, from one place: `settings.department` (Rust resolves it against the machine's
 * registry, so it is always one this machine may use). `choose` saves it through the settings
 * action, which refuses one the profile does not authorize, and tells the other components in the
 * same tab with a window event. Outside the app there are no settings and `dept` stays empty.
 */
export function useDepartment(): {
  dept: string;
  /** True when the department changed; false when the profile does not authorize it. */
  choose: (name: string) => Promise<boolean>;
} {
  const [dept, setDept] = useState("");

  useEffect(() => {
    let live = true;
    const read = () =>
      loadSettings()
        .then((s) => live && setDept(s?.department ?? ""))
        .catch(() => {});
    void read();
    window.addEventListener(DEPT_EVENT, read);
    return () => {
      live = false;
      window.removeEventListener(DEPT_EVENT, read);
    };
  }, []);

  const choose = useCallback(async (name: string) => {
    const now = await loadSettings();
    if (!now) return false;
    try {
      await saveSettings({ ...now, department: name });
    } catch {
      return false; // not authorized: nothing changes
    }
    window.dispatchEvent(new Event(DEPT_EVENT));
    return true;
  }, []);

  return { dept, choose };
}
