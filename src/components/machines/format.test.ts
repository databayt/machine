// node --test src/components/machines/format.test.ts
import assert from "node:assert/strict"
import { test } from "node:test"

import type { MachineStatus } from "../network/types.ts"
import { ago, buildParams, cpuName, DEFAULT_FORM, duration, gateLine, gb, poolTotals, summary } from "./format.ts"

const GIB = 1024 ** 3

test("sizes read as whole GB, one decimal under ten", () => {
  assert.equal(gb(48 * GIB), "48 GB")
  assert.equal(gb(7.5 * GIB), "7.5 GB")
  assert.equal(gb(undefined), "?")
})

test("processor names are short", () => {
  assert.equal(cpuName("Apple M4 Pro"), "M4 Pro")
  assert.equal(cpuName("AMD Ryzen 9 7950X 16-Core Processor"), "Ryzen 9 7950X")
  assert.equal(cpuName("Intel(R) Core(TM) i7-1165G7 @ 2.80GHz"), "i7-1165G7")
})

test("the card line, in both languages", () => {
  const r = {
    cpu: { model: "Apple M4 Pro", cores: 14 },
    ramTotal: 48 * GIB,
    gpu: [{ name: "Apple M4 Pro", cores: 20 }],
    diskFree: 120 * GIB,
    power: { onAc: true },
  }
  assert.equal(summary(r, false), "M4 Pro · 48 GB · 20-core GPU · 120 GB free · on power")
  assert.match(summary(r, true), /^M4 Pro · 48 GB · 20 نواة رسومية · 120 GB متاح · على الكهرباء$/)
  assert.equal(summary({ power: { onAc: false, batteryPercent: 54 } }, false), "battery 54%")
  assert.equal(summary(null, false), "No readings yet")
  assert.equal(summary({ gpu: [{ name: "NVIDIA GeForce RTX 4090", vramBytes: 24 * GIB }] }, false), "RTX 4090")
})

const m = (over: Partial<MachineStatus>): MachineStatus => ({
  login: "x", name: "X", os: "macos", departments: [], lastSeenAt: null, healthOk: null, currentTask: null,
  resources: null, share: null, online: true, readinessPercent: null, gate: null, ...over,
})

test("the pool row counts only machines that are online", () => {
  const t = poolTotals([
    m({ resources: { ramFree: 10 * GIB, gpu: [{ name: "a" }] }, share: { enabled: true, kinds: ["encode"], idleMinutes: 10 } }),
    m({ resources: { ramFree: 6 * GIB } }),
    m({ online: false, resources: { ramFree: 100 * GIB, gpu: [{ name: "b" }, { name: "c" }] }, share: { enabled: true, kinds: [], idleMinutes: 10 } }),
  ])
  assert.deepEqual(t, { online: 2, sharing: 1, ramFree: 16 * GIB, gpus: 1 })
})

test("the gate line", () => {
  assert.equal(gateLine(null), null)
  assert.deepEqual(gateLine({ pass: true, failing: [], at: "t" }), { pass: true, left: 0 })
  assert.deepEqual(gateLine({ pass: false, failing: ["a", "b"], at: "t" }), { pass: false, left: 2 })
})

test("times", () => {
  const now = Date.parse("2026-11-22T12:00:00Z")
  assert.equal(ago("2026-11-22T11:55:00Z", now, "en"), "5 minutes ago")
  assert.equal(ago(null, now, "en"), "")
  assert.equal(duration(30), "30 s")
  assert.equal(duration(125), "2 min")
  assert.equal(duration(3900), "1 h 5 min")
})

test("job parameters are built and checked with kun's schemas", () => {
  assert.deepEqual(buildParams("encode", DEFAULT_FORM), { preset: "h264-web" })
  assert.deepEqual(buildParams("transcribe", { ...DEFAULT_FORM, language: " AR ", model: "small" }), { format: "srt", model: "small", language: "ar" })
  assert.deepEqual(buildParams("transcribe", DEFAULT_FORM), { format: "srt" })
  assert.equal(buildParams("transcribe", { ...DEFAULT_FORM, language: "arabic" }), null)
  assert.deepEqual(buildParams("render", { ...DEFAULT_FORM, frameStart: "1", frameEnd: "24" }), { engine: "eevee", frameStart: 1, frameEnd: 24, format: "mp4" })
  assert.equal(buildParams("render", { ...DEFAULT_FORM, frameStart: "10", frameEnd: "5" }), null)
  assert.equal(buildParams("render", { ...DEFAULT_FORM, frameStart: "", frameEnd: "x" }), null)
})
