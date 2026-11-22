import assert from "node:assert/strict"
import { test } from "node:test"

import { doctorDetail, doctorNote, doctorSteps, isDoctorRefusal } from "./doctor.ts"

test("doctor reasons are the gate steps, in order, and nothing else", () => {
  assert.deepEqual(doctorSteps(["doctor:launcher.c", "repo:kun", "doctor:pack.tech"]), ["launcher.c", "pack.tech"])
  assert.deepEqual(doctorSteps(["after:x", "wip"]), [])
})

test("the server's doctor refusal is recognised with or without the Error prefix", () => {
  assert.equal(isDoctorRefusal("doctor: gh.auth, pack.tech"), true)
  assert.equal(isDoctorRefusal("Error: doctor: stale"), true)
  assert.equal(isDoctorRefusal("held: ali"), false)
  assert.equal(isDoctorRefusal("doctors are in"), false)
  assert.equal(doctorDetail("doctor: gh.auth, pack.tech"), "gh.auth, pack.tech")
  assert.equal(doctorDetail("doctor"), "")
})

test("orange shows the extras count, unknown says checking, green and red say nothing here", () => {
  assert.deepEqual(doctorNote({ doctorExtras: 2, doctorChecking: false }, false), { text: "Doctor: 2 extras missing", link: true })
  assert.deepEqual(doctorNote({ doctorExtras: 1, doctorChecking: false }, false), { text: "Doctor: 1 extra missing", link: true })
  assert.deepEqual(doctorNote({ doctorExtras: 0, doctorChecking: true }, false), { text: "Checking…", link: false })
  assert.equal(doctorNote({ doctorExtras: 0, doctorChecking: false }, false), null)
})
