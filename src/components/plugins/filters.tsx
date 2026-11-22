"use client"

// plugins + apps — the two filters they share: the search field and the department pills.
import { Search } from "lucide-react"
import { useEffect, useState } from "react"

import { cn } from "@/lib/utils"

import type { Copy } from "../chat/config"
import { useDepartment } from "../menu/use-department"
import { getProfile } from "../profile/actions"
import { useInMachine } from "../shell/content"
import { copy } from "./config"
import { DEPARTMENTS } from "./registry"

export interface DeptOption {
  name: string
  title: Copy
}

/** The pills: the profile's departments inside the app, else the eight. The default pill is the saved department. */
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

  // Page state only: picking a pill never changes the saved department.
  // The saved department, else the machine ID's default, else All.
  const mine = dept || home
  const value = picked ?? (options.some((o) => o.name === mine) ? mine : "all")
  return { options, value, set: setPicked }
}

const pill = (on: boolean) =>
  cn("rounded-full px-4 py-2 text-base transition-colors", on ? "bg-foreground text-background" : "bg-muted text-foreground hover:bg-accent")

export function DeptPills({ options, value, onChange, t }: { options: DeptOption[]; value: string; onChange: (name: string) => void; t: (c: Copy) => string }) {
  return (
    <div role="group" aria-label={t(copy.department)} className="flex flex-wrap gap-3">
      {[{ name: "all", title: copy.all }, ...options].map((o) => (
        <button key={o.name} type="button" aria-pressed={value === o.name} onClick={() => onChange(o.name)} className={pill(value === o.name)}>
          {t(o.title)}
        </button>
      ))}
    </div>
  )
}

/** Full width, white in both themes. */
export function SearchField({ value, onChange, placeholder }: { value: string; onChange: (v: string) => void; placeholder: string }) {
  return (
    <label className="border-border focus-within:ring-ring/50 flex h-10 w-full items-center gap-2 rounded-full border bg-white px-4 focus-within:ring-2">
      <Search className="text-muted-foreground size-4 shrink-0" aria-hidden />
      <input
        type="search"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder={placeholder}
        aria-label={placeholder}
        className="placeholder:text-muted-foreground w-full bg-transparent text-sm text-black outline-none"
      />
    </label>
  )
}
