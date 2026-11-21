"use client"

import { useEffect, useState } from "react"

import type { Locale } from "@/components/local/config"
import { tr } from "../chat/config"
import { copy } from "../menu/config"
import { DepartmentList } from "../menu/departments"
import { useDepartment } from "../menu/use-department"
import { getProfile, type MachineProfile } from "../profile/actions"
import { Section } from "../shell/page"

/**
 * Settings → General: the department this machine works in. The machine ID (kun registry) lists the
 * authorized departments and sets the default. With one it is read-only; with several, choosing one
 * saves it and the menu, chat and sidebar follow at once. Rust refuses an unauthorized one anyway.
 */
export function DepartmentSection({ lang }: { lang: Locale }) {
  const isAr = lang === "ar"
  const t = tr(isAr)
  const [profile, setProfile] = useState<MachineProfile | null>(null)
  const { dept, choose } = useDepartment()

  useEffect(() => {
    getProfile().then(setProfile).catch(() => setProfile(null))
  }, [])

  if (!profile || profile.departments.length === 0) return null
  const only = profile.departments.length === 1 ? profile.departments[0] : null

  return (
    <Section title={t(copy.department)}>
      {only ? (
        <>
          <p className="text-lg font-semibold tracking-tight">{(isAr && only.title_ar) || only.title}</p>
          <p className="text-sm text-muted-foreground">{t(copy.departmentSetBy)}</p>
        </>
      ) : (
        <>
          <DepartmentList
            isAr={isAr}
            profile={profile}
            selected={dept}
            label={t(copy.department)}
            onSelect={(name) => void choose(name)}
          />
          <p className="text-sm text-muted-foreground">{t(copy.departmentPick)}</p>
        </>
      )}
    </Section>
  )
}
