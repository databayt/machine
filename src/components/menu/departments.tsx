import { cn } from "@/lib/utils";
import type { MachineProfile } from "../profile/types";

/** Column 1: the departments. Big bold; the selected one is foreground, the rest muted. A scroll row on phones. */
export function Departments({
  isAr,
  profile,
  selected,
  label,
  onSelect,
  first,
}: {
  isAr: boolean;
  profile: MachineProfile;
  selected: string;
  label: string;
  onSelect: (name: string) => void;
  first: React.Ref<HTMLButtonElement>;
}) {
  return (
    <section className="min-w-0">
      <ul
        role="listbox"
        aria-label={label}
        className="-mx-6 flex gap-5 overflow-x-auto px-6 pb-1 lg:mx-0 lg:block lg:overflow-visible lg:px-0"
      >
        {profile.departments.map((d) => {
          const on = d.name === selected;
          return (
            <li key={d.name} role="presentation" className="shrink-0">
              <button
                ref={on ? first : undefined}
                type="button"
                role="option"
                aria-selected={on}
                onClick={() => onSelect(d.name)}
                className={cn(
                  "block whitespace-nowrap rounded-md py-1 text-start text-lg font-semibold leading-tight tracking-tight outline-none transition-colors focus-visible:ring-2 focus-visible:ring-ring lg:text-2xl",
                  on ? "text-foreground" : "text-muted-foreground hover:text-foreground",
                )}
              >
                {(isAr && d.title_ar) || d.title}
              </button>
            </li>
          );
        })}
      </ul>
    </section>
  );
}
