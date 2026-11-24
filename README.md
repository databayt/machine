# machine

Kun on every teammate's computer: chat with an agent CLI, voice, the catalog, repos and tasks.
A Tauri 2 shell (`machine/`) around a **static-export Next.js UI** (`src/`) that is bundled into
the app — no server, no remote origin. Part of the [databayt](https://github.com/databayt) stack;
the engine config it installs and the API it talks to (`KUN_API`, default `https://kun.databayt.org`)
live in [kun](https://github.com/databayt/kun). Docs: [kun.databayt.org/docs/machine](https://kun.databayt.org/en/docs/machine).

## Develop

```bash
pnpm install
pnpm dev            # the UI alone, http://localhost:3000/ar/ (a browser shows "open in the app")
pnpm tauri dev      # the app: starts `pnpm dev`, opens the window on /ar/
```

## Build

```bash
pnpm build                    # static export to out/ (ar + en, every route)
pnpm tauri build              # installers (build.rs + out/ -> machine/target/release/bundle)
pnpm tauri build --debug --no-bundle   # just prove the shell compiles
pnpm typecheck
cd machine && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
```

Rust 1.90, Node 22, pnpm 10.

## Routes

The app is a drill-down: the sidebar is a tree, each click swaps the whole screen to the next level, and
the sidebar shows the current path under the active parent (it is the breadcrumb). The work area is bare:
no title, no toolbar, only content. All paths are under `/{ar,en}/`; levels below a route are query
params, read in the client (`useSearchParams`), since a static export has no dynamic segments.

| Parent (sidebar) | Level 1 | Level 2 | Level 3 |
| --- | --- | --- | --- |
| Sessions | `/sessions/` | | |
| Brainstorm (app only) | `/brainstorm/` | `/brainstorm/?item=<seat>` (the live site: ChatGPT, DeepSeek, GLM, Gemini) | |
| Plugins | `/plugins/` | `/plugins/?item=<id>` | |
| Apps | `/apps/` | `/apps/?item=<stepId>` | |
| Library | `/library/` | `/library/?dir=<skills\|agents\|rules\|hooks\|departments>` | `/library/?dir=<dir>&item=<type:name>` |
| Setup | `/setup/` | `/setup/?stage=<stageId>` (not foundation, agents, tools: those are Apps) | `/setup/?stage=<stageId>&step=<stepId>` |
| Settings | `/settings/` | `/settings/?view=general`, `/config/`, `/health/`, `/engine/` | |

The chat is `/` (`?s=<session>`, `?prompt=`, `?task=`). The tree lives in `src/components/shell/nav.ts`.

## Feature-colocated Rust

A feature is one folder, `src/components/<feature>/`, holding its UI (`content.tsx`, `actions.ts`,
`types.ts`) **and** its Rust (`rust.rs`, split siblings, `rust_test.rs`, optional `capability.json`).
`machine/build.rs` scans `../src/components/*/rust.rs` (folders without one — `ui`, `atom`, `local` —
are skipped), turns each into a module and, if it declares `#[tauri::command]`s, an inlined Tauri
plugin with `allow-*` permissions. The shell never names a feature. ts-rs writes each feature's
`types.ts` next to its `rust.rs` (`machine/.cargo/config.toml`). The generated capability grants the
`main` window local IPC only. `MACHINE_ONLY=driver,chat cargo build` limits a build to some features.

## License

SSPL-1.0 — see `LICENSE`.
