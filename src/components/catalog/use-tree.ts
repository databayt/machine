"use client"

import { useCallback, useEffect, useRef, useState } from "react"

import { tree } from "./actions"
import type { Entry } from "./types"

/** What is under `~/.claude`; `run` does one action on one id, then reloads. */
export function useTree() {
  const [entries, setEntries] = useState<Entry[] | null>(null)
  const [error, setError] = useState<string | null>(null)
  /** The id an action is working on (null when idle). */
  const [busy, setBusy] = useState<string | null>(null)
  const [actionError, setActionError] = useState<string | null>(null)
  const alive = useRef(true)

  const reload = useCallback(async () => {
    try {
      const e = await tree()
      if (!alive.current) return
      setEntries(e)
      setError(null)
    } catch (e) {
      if (alive.current) setError(String(e))
    }
  }, [])

  useEffect(() => {
    alive.current = true
    void reload()
    return () => {
      alive.current = false
    }
  }, [reload])

  const run = useCallback(
    async (id: string, job: () => Promise<unknown>) => {
      setBusy(id)
      setActionError(null)
      try {
        await job()
      } catch (e) {
        if (alive.current) setActionError(String(e))
      }
      await reload()
      if (alive.current) setBusy(null)
    },
    [reload],
  )

  return { entries, error, reload, busy, actionError, run }
}

export type TreeState = ReturnType<typeof useTree>
