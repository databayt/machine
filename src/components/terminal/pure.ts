// terminal — the pure part of the views (tested in pure.test.ts).
import type { TerminalChunk } from "./types"

/**
 * Which part of a live chunk is new, given that `seen` bytes of the session were already written:
 * null when all of it was, else the bytes to write and the new `seen`.
 */
export function freshPart(chunk: TerminalChunk, bytes: Uint8Array, seen: number): { bytes: Uint8Array; seen: number } | null {
  if (chunk.end <= seen) return null
  const start = chunk.end - bytes.length
  return { bytes: start >= seen ? bytes : bytes.subarray(seen - start), seen: chunk.end }
}
