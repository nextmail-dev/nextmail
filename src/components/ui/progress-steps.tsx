import { Check } from "lucide-react";

import { cn } from "@/lib/utils";

interface ProgressStepsProps {
  label: string;
  items: string[];
  activeIndex: number;
}

export function ProgressSteps({ label, items, activeIndex }: ProgressStepsProps) {
  return (
    <nav className="flex flex-col gap-4" aria-label={label}>
      {items.map((item, index) => (
        <div
          className={cn(
            "text-[length:var(--ui-font-control)] flex items-center gap-3 font-semibold text-muted-foreground/70",
            index <= activeIndex && "text-foreground",
          )}
          key={item}
        >
          <span
            className={cn(
              "text-[length:var(--ui-font-caption)] flex size-6 items-center justify-center rounded-sm border-0 bg-muted",
              index <= activeIndex && "bg-primary text-primary-foreground",
            )}
          >
            {index < activeIndex ? <Check size={12} /> : index + 1}
          </span>
          <span>{item}</span>
        </div>
      ))}
    </nav>
  );
}
