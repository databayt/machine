import { z } from "zod"

// Mirrors settings/rust.rs; Rust re-validates every save.
export const settingsSchema = z.object({
  // The machine profile (machines/<login>.json). Empty = no profile = full access.
  login: z.string().regex(/^[a-z0-9-]{0,32}$/),
  person: z
    .string()
    .trim()
    .max(80)
    .refine((s) => !/\p{Cc}/u.test(s)),
  // Empty = the machine's registry default. Rust refuses one the profile does not authorize.
  department: z.union([z.literal(""), z.string().regex(/^[A-Za-z0-9_][A-Za-z0-9_.-]{0,63}$/).refine((s) => !s.includes(".."))]),
  // The composer's repository per department; carried through untouched by the settings form.
  repo_by_department: z.record(z.string(), z.string()),
  driver: z.enum(["claude", "codex", "opencode"]),
  // Opt-in pickup of `run:auto` tasks (settings/rust.rs `auto_run`, serialized as autoRun).
  autoRun: z.boolean(),
})

export type SettingsInput = z.infer<typeof settingsSchema>
