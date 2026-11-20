"use client";

import { useCallback, useEffect, useState } from "react";

import type { MachineProfile } from "../profile/types";
import { DEPT_KEY } from "./config";

const DEPT_EVENT = "kun:dept";

/**
 * The selected department: the remembered choice (localStorage, `DEPT_KEY`) if the profile still has
 * it, else the profile's first. The menu and the app sidebar both read it, so a pick in one reaches
 * the other in the same tab through a window event.
 */
export function useDepartment(profile: Pick<MachineProfile, "departments"> | null): {
  dept: string;
  choose: (name: string) => void;
} {
  const [dept, setDept] = useState("");

  useEffect(() => {
    if (!profile?.departments.length) return;
    const names = profile.departments.map((d) => d.name);
    const read = () => {
      let saved = "";
      try {
        saved = localStorage.getItem(DEPT_KEY) ?? "";
      } catch {}
      setDept(names.includes(saved) ? saved : names[0]);
    };
    read();
    window.addEventListener(DEPT_EVENT, read);
    return () => window.removeEventListener(DEPT_EVENT, read);
  }, [profile]);

  const choose = useCallback((name: string) => {
    setDept(name);
    try {
      localStorage.setItem(DEPT_KEY, name);
    } catch {}
    window.dispatchEvent(new Event(DEPT_EVENT));
  }, []);

  return { dept, choose };
}
