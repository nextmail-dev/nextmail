import type {
  FormHTMLAttributes,
  HTMLAttributes,
  PropsWithChildren,
} from "react";

import { cn } from "@/lib/utils";

export function AppShell({ className, ...props }: HTMLAttributes<HTMLElement>) {
  return <main className={cn("size-full bg-background text-foreground", className)} {...props} />;
}

export function Page({ className, ...props }: HTMLAttributes<HTMLElement>) {
  return <section className={cn("min-w-0", className)} {...props} />;
}

const stackGaps = {
  xs: "gap-1.5",
  sm: "gap-2.5",
  md: "gap-4",
  lg: "gap-6",
  xl: "gap-9",
};

type StackProps = HTMLAttributes<HTMLDivElement> & {
  gap?: keyof typeof stackGaps;
};

export function Stack({ className, gap = "md", ...props }: StackProps) {
  return <div className={cn("flex min-w-0 flex-col", stackGaps[gap], className)} {...props} />;
}

export function Inline({ className, ...props }: HTMLAttributes<HTMLDivElement>) {
  return <div className={cn("flex min-w-0 items-center gap-2.5", className)} {...props} />;
}

export function Form({ className, ...props }: FormHTMLAttributes<HTMLFormElement>) {
  return <form className={cn("w-full", className)} {...props} />;
}

export function ScreenReaderText({ children }: PropsWithChildren) {
  return <span className="sr-only">{children}</span>;
}

