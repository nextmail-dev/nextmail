import { cn } from "@/lib/utils";

export function Progress({ value, className }: { value: number; className?: string }) {
  const normalized = Math.min(100, Math.max(0, value));
  return (
    <div
      className={cn("h-1.5 w-full overflow-hidden rounded-xs bg-muted", className)}
      role="progressbar"
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={Math.round(normalized)}
    >
      <div
        className="h-full bg-primary transition-[width] duration-300"
        style={{ width: `${normalized}%` }}
      />
    </div>
  );
}
