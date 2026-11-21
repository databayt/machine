import { Geist_Mono as FontMono, Geist as FontSans, Rubik } from "next/font/google"

import { cn } from "@/lib/utils"

export const fontSans = FontSans({ subsets: ["latin"], variable: "--font-sans" })

export const fontMono = FontMono({ subsets: ["latin"], variable: "--font-mono", weight: ["400"] })

// Rubik carries the Arabic script.
export const fontRubik = Rubik({
  subsets: ["latin", "arabic"],
  weight: ["300", "400", "500", "600", "700"],
  variable: "--font-rubik",
  display: "swap",
})

export const fontVariables = cn(fontSans.variable, fontMono.variable, fontRubik.variable)

/**
 * Headlines, as in the marketing repo's hero: English in Geist 700 at -0.04em, Arabic in the
 * Thmanyah serif (Rubik behind it until the face loads).
 */
export const headline =
  "font-sans font-bold tracking-[-0.04em] rtl:font-['thmanyah_serif_display',var(--font-rubik)] rtl:tracking-normal"
