// shell — the one geometry the mega menu and every page share, so the sidebar never jumps between them.

/**
 * Large screens: a 12rem sidebar column, a gap, and the work area, top-aligned, starting at the
 * content edge (46px; the hamburger's icon lines up with it). Used by the menu's `nav` and `ShellFrame`.
 */
export const SHELL_GRID =
  "lg:grid lg:grid-cols-[12rem_minmax(0,1fr)] lg:content-start lg:items-start lg:gap-x-28 lg:ps-[46px]";

/**
 * The offset from the top on large screens, the same for the mega menu (the first page you see)
 * and every page, so nothing starts higher or lower as you move between them.
 */
export const SHELL_TOP = "lg:pt-32";
