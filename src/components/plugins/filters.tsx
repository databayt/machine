"use client"

// plugins, apps and machines — the department filter they share: the home screen's Department select.
import { useEffect, useState } from "react"

import type { Copy } from "../chat/config"
import { DepartmentSelect } from "../chat/department-select"
import { useDepartment } from "../menu/use-department"
import { getProfile } from "../profile/actions"
import { useInMachine } from "../shell/content"
import { copy } from "./config"
import { DEPARTMENTS } from "./registry"

export interface DeptOption {
  name: string
  title: Copy
}

/** The options: the profile's departments inside the app, else the eight. The default is the machine's department, else All. */
export function useDeptFilter(): { options: DeptOption[]; value: string; set: (name: string) => void } {
  const inApp = useInMachine()
  const { dept } = useDepartment()
  const [options, setOptions] = useState<DeptOption[]>(DEPARTMENTS)
  const [picked, setPicked] = useState<string | null>(null)
  const [home, setHome] = useState("")

  useEffect(() => {
    if (!inApp) return
    getProfile()
      .then((p) => {
        if (p?.department) setHome(p.department)
        if (p && p.departments.length) setOptions(p.departments.map((d) => ({ name: d.name, title: { en: d.title, ar: d.title_ar || d.title } })))
      })
      .catch(() => {})
  }, [inApp])

  // Page state only: picking one never changes the saved department.
  // The machine's department (settings), else the machine ID's default, else All.
  const mine = dept || home
  const value = picked ?? (options.some((o) => o.name === mine) ? mine : "all")
  return { options, value, set: setPicked }
}

/** The home screen's Department select, with an "All" row, in the page's top-left. */
export function DeptFilter({ lang, options, value, onChange }: { lang: string; options: DeptOption[]; value: string; onChange: (name: string) => void }) {
  return <DepartmentSelect isAr={lang === "ar"} options={options} value={value} onChange={onChange} all={copy.all} />
}
