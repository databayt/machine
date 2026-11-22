// Reproducible logos: `pnpm logos` rewrites public/logos/ for every app and plugin that has an
// original mark, then optimizes the whole folder. No React, no network beyond the vendor fetches.
//
// Sources (the order of trust):
//   1. Simple Icons (`simple-icons`, the current release): the single-path mark, coloured with its
//      brand hex from the package data.
//   2. Simple Icons 13 (`simple-icons-v13`, pinned): marks later removed from the package at the
//      brands' request (OpenAI, Adobe, Canva). Same mark, same hex, kept so the tiles stay original.
//   3. The vendor's own asset, fetched below; the URL is the record of where it came from.
// No official mark anywhere (Homebrew has one, Colima, Tesseract and FilmCraft do not): the app
// keeps its lettered tile, there is no entry here.
//
// Then: svgo on every .svg (precision 2, metadata and editor cruft out, viewBox kept) and every
// .png capped at 64x64 (macOS `sips`). A .png stays only where the vendor ships no SVG.
import { execFileSync } from "node:child_process"
import { mkdirSync, readdirSync, readFileSync, statSync, writeFileSync } from "node:fs"
import { dirname, join } from "node:path"
import { fileURLToPath } from "node:url"

import * as current from "simple-icons"
import * as v13 from "simple-icons-v13"
import { optimize } from "svgo"

const OUT = join(dirname(fileURLToPath(import.meta.url)), "..", "public", "logos")
mkdirSync(OUT, { recursive: true })

/** file name → [package, slug, colour override]. */
const SIMPLE = {
  "homebrew.svg": [current, "homebrew"],
  "git.svg": [current, "git"],
  "uv.svg": [current, "uv"],
  "ffmpeg.svg": [current, "ffmpeg"],
  "tmux.svg": [current, "tmux"],
  "nodejs.svg": [current, "nodedotjs"],
  "pnpm.svg": [current, "pnpm"],
  "claude-code.svg": [current, "claudecode"],
  "opencode.svg": [current, "opencode"],
  "obs.svg": [current, "obsstudio"],
  "webstorm.svg": [current, "webstorm"],
  // v13: OpenAI's mark is black today; the v13 hex is the old purple, so it is overridden.
  "openai.svg": [v13, "openai", "000000"],
  "adobe.svg": [v13, "adobe"],
  "canva.svg": [v13, "canva"],
}

const bySlug = (pkg, slug) => {
  const icon = Object.values(pkg).find((i) => i && typeof i === "object" && i.slug === slug)
  if (!icon) throw new Error(`simple-icons has no "${slug}"`)
  return icon
}

for (const [file, [pkg, slug, color]] of Object.entries(SIMPLE)) {
  const i = bySlug(pkg, slug)
  writeFileSync(
    join(OUT, file),
    `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="#${color ?? i.hex}"><path d="${i.path}"/></svg>`,
  )
}

/** Vendor assets: file → [url, how to clean it]. */
const VENDOR = {
  // jq's own site mark. Its dark-mode rule inverts the fills; the app themes the backing instead.
  "jq.svg": ["https://jqlang.org/jq.svg", (s) => s.replace(/<style>[\s\S]*?<\/style>/, "")],
  // Antigravity's favicon: the gradient mark on a white disc; the disc goes, the viewBox crops to the mark.
  "antigravity.svg": [
    "https://antigravity.google/favicon.svg",
    (s) =>
      s
        .replace(/<style>[\s\S]*?<\/style>/, "")
        .replace(/<circle[^>]*\/>/, "")
        .replace(/width="120" height="120" viewBox="0 0 136 136"/, 'viewBox="26 26 83 83"'),
  ],
  // Comfy Org's favicon (the yellow C on its violet tile).
  "comfyui.svg": ["https://comfy.org/favicon.svg"],
  // Upstash's Context7 icon, from its repository.
  "context7.svg": ["https://raw.githubusercontent.com/upstash/context7/HEAD/public/context7-icon.svg"],
  // No SVG from these vendors: their app icons, capped at 64x64 below.
  "vscode.png": ["https://raw.githubusercontent.com/microsoft/vscode/main/resources/linux/code.png"],
  "hermes.png": ["https://raw.githubusercontent.com/NousResearch/hermes-agent/HEAD/apps/bootstrap-installer/src-tauri/icons/128x128.png"],
  "artcraft.png": ["https://raw.githubusercontent.com/storytold/artcraft/HEAD/crates/desktop/artcraft/icons/64x64.png"],
}

for (const [file, [url, clean]] of Object.entries(VENDOR)) {
  const res = await fetch(url)
  if (!res.ok) throw new Error(`${url}: ${res.status}`)
  if (file.endsWith(".png")) writeFileSync(join(OUT, file), Buffer.from(await res.arrayBuffer()))
  else writeFileSync(join(OUT, file), clean ? clean(await res.text()) : await res.text())
}

// Optimize everything in the folder.
const SVGO = {
  multipass: true,
  floatPrecision: 2,
  plugins: ["preset-default", "removeDimensions"], // svgo 4: preset-default keeps the viewBox
}
for (const f of readdirSync(OUT)) {
  const p = join(OUT, f)
  if (f.endsWith(".svg")) writeFileSync(p, optimize(readFileSync(p, "utf8"), { path: p, ...SVGO }).data)
  else if (f.endsWith(".png")) execFileSync("sips", ["-Z", "64", p], { stdio: "ignore" })
}

const total = readdirSync(OUT).reduce((n, f) => n + statSync(join(OUT, f)).size, 0)
console.log(`public/logos: ${readdirSync(OUT).length} files, ${total} bytes`)
