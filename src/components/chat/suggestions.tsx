"use client";

import { useState } from "react";

import {
  ActivityIcon,
  BadgeCheckIcon,
  BoxIcon,
  CameraIcon,
  ClapperboardIcon,
  CloudUploadIcon,
  EyeIcon,
  FileTextIcon,
  FilePlusIcon,
  FilmIcon,
  GlobeIcon,
  ImagesIcon,
  InboxIcon,
  LayoutGridIcon,
  LightbulbIcon,
  ListChecksIcon,
  MicIcon,
  PenLineIcon,
  PlayIcon,
  ScissorsIcon,
  ScrollTextIcon,
  SearchIcon,
  SendIcon,
  ShieldCheckIcon,
  VideoIcon,
  WorkflowIcon,
  WrenchIcon,
  XIcon,
  ZapIcon,
  type LucideIcon,
} from "lucide-react";

import { type HomeOption, iconKey } from "./options";

/** The lucide icons a formula may name (`icon` in its header), by `iconKey`; anything else falls back. */
const NAMED: Record<string, LucideIcon> = {
  play: PlayIcon,
  film: FilmIcon,
  scrolltext: ScrollTextIcon,
  clapperboard: ClapperboardIcon,
  wrench: WrenchIcon,
  activity: ActivityIcon,
  video: VideoIcon,
  penline: PenLineIcon,
  filetext: FileTextIcon,
  fileplus: FilePlusIcon,
  globe: GlobeIcon,
  lightbulb: LightbulbIcon,
  listchecks: ListChecksIcon,
  zap: ZapIcon,
  workflow: WorkflowIcon,
  camera: CameraIcon,
  images: ImagesIcon,
  inbox: InboxIcon,
  layoutgrid: LayoutGridIcon,
  mic: MicIcon,
  scissors: ScissorsIcon,
  search: SearchIcon,
  send: SendIcon,
  shieldcheck: ShieldCheckIcon,
  badgecheck: BadgeCheckIcon,
  box: BoxIcon,
  eye: EyeIcon,
  cloudupload: CloudUploadIcon,
};

/** The pack's quick actions have no icon of their own: one by what they ask for. */
const GUESSED: [RegExp, LucideIcon][] = [
  [/فحص الأدوات|tools check/i, WrenchIcon],
  [/متابعة|continue/i, PlayIcon],
  [/تعريفي|intro|film/i, ClapperboardIcon],
  [/مكتب|desk|status|جاهزية/i, ActivityIcon],
  [/فيديو|ريلز|video|reel|لقطات|screenshot/i, VideoIcon],
  [/منشور|اكتب|post|write|draft|سكربت|script/i, PenLineIcon],
  [/عرض|سعر|quote|proposal|pric/i, FileTextIcon],
  [/عملاء|مدارس|leads|find|search|ابحث/i, GlobeIcon],
];

/** The option's own icon when it names a known one, else a guess from its words, else a bulb. */
export const iconFor = (o: Pick<HomeOption, "icon" | "label" | "labelEn" | "prompt">): LucideIcon =>
  NAMED[iconKey(o.icon)] ??
  GUESSED.find(([re]) => re.test(`${o.labelEn} ${o.label} ${o.prompt ?? ""}`))?.[1] ??
  LightbulbIcon;

/**
 * The options under the empty composer: a plain list of at most three, no borders, no dividers, no
 * hover fill. The end of a row is empty until it is hovered; then a ✕ appears that clears that option.
 * A tap writes its prompt into the box to edit and send.
 */
export function Suggestions({
  options,
  isAr,
  onPick,
  onDismiss,
  disabled,
}: {
  options: HomeOption[];
  isAr: boolean;
  onPick: (option: HomeOption) => void;
  onDismiss?: (option: HomeOption) => void;
  disabled?: boolean;
}) {
  // Options the person cleared with ✕ — gone for this visit.
  const [cleared, setCleared] = useState<string[]>([]);
  const shown = options.filter((o) => !cleared.includes(o.key));
  if (shown.length === 0) return null;
  return (
    <ul className="flex w-full flex-col">
      {shown.map((o) => {
        const Icon = iconFor(o);
        return (
          <li key={o.key} className="group flex items-center justify-between gap-2">
            <button
              type="button"
              onClick={() => onPick(o)}
              disabled={disabled}
              className="flex min-w-0 flex-1 items-center gap-4 px-2 py-3.5 text-start text-[17px] text-muted-foreground transition-colors hover:text-foreground disabled:opacity-50"
            >
              <Icon className="size-5 shrink-0" strokeWidth={1.5} aria-hidden />
              <span dir="auto" className="truncate">
                {o.label}
              </span>
            </button>
            <button
              type="button"
              onClick={() => {
                setCleared((c) => [...c, o.key]);
                onDismiss?.(o);
              }}
              aria-label={isAr ? `إخفاء: ${o.label}` : `Clear: ${o.label}`}
              className="flex size-8 shrink-0 items-center justify-center rounded-full text-muted-foreground opacity-0 transition-opacity hover:text-foreground focus-visible:opacity-100 group-hover:opacity-100"
            >
              <XIcon className="size-4" aria-hidden />
            </button>
          </li>
        );
      })}
    </ul>
  );
}
