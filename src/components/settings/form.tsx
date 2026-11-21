"use client"

import { useState } from "react"

import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { cn } from "@/lib/utils"
import { loadSettings, saveSettings } from "./actions"
import type { Settings } from "./types"
import { settingsSchema } from "./validation"

interface Props {
  isAr: boolean
  initial: Settings
}

export function SettingsForm({ isAr, initial }: Props) {
  const [values, setValues] = useState<Settings>(initial)
  const [message, setMessage] = useState<{ ok: boolean; text: string } | null>(null)
  const [busy, setBusy] = useState(false)

  async function onSubmit(e: React.FormEvent) {
    e.preventDefault()
    const parsed = settingsSchema.safeParse(values)
    if (!parsed.success) {
      setMessage({ ok: false, text: isAr ? "تحقق من الحقول." : "Check the fields." })
      return
    }
    setBusy(true)
    try {
      // The department is chosen elsewhere on this page: keep whatever is stored now.
      const fresh = (await loadSettings()) ?? parsed.data
      setValues(await saveSettings({ ...fresh, person: parsed.data.person }))
      setMessage({ ok: true, text: isAr ? "تم الحفظ." : "Saved." })
    } catch (err) {
      setMessage({ ok: false, text: String(err) })
    } finally {
      setBusy(false)
    }
  }

  return (
    <form onSubmit={onSubmit} className="space-y-6">
      <div className="space-y-2">
        <Label htmlFor="person">{isAr ? "الاسم" : "Your name"}</Label>
        <Input id="person" className="h-12 rounded-full bg-muted px-5 text-base shadow-none" value={values.person} maxLength={80} onChange={(e) => setValues({ ...values, person: e.target.value })} />
      </div>

      <div className="flex items-center gap-3">
        <Button type="submit" disabled={busy} className="h-12 rounded-full px-6 text-base">{isAr ? "حفظ" : "Save"}</Button>
        {message && <span className={cn("text-sm", message.ok ? "text-muted-foreground" : "text-destructive")}>{message.text}</span>}
      </div>
    </form>
  )
}
