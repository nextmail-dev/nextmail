import { Slot } from "@radix-ui/react-slot";
import { cva, type VariantProps } from "class-variance-authority";
import { forwardRef, type ButtonHTMLAttributes } from "react";

import { cn } from "@/lib/utils";
import { Spinner } from "./spinner";

const buttonVariants = cva(
  "text-[length:var(--ui-font-control)] inline-flex shrink-0 cursor-default items-center justify-center gap-2 whitespace-nowrap border-0 font-semibold transition-[color,background-color,filter,opacity,box-shadow] duration-150 outline-none focus-visible:ring-1 focus-visible:ring-inset focus-visible:ring-ring/70 disabled:pointer-events-none disabled:opacity-50",
  {
    variants: {
      variant: {
        primary:
          "bg-primary [background:var(--primary-gradient)] text-primary-foreground shadow-[var(--shadow-primary)] hover:brightness-[1.04] active:brightness-[0.98]",
        secondary:
          "border border-border/80 bg-secondary text-secondary-foreground shadow-[var(--shadow-control)] hover:bg-accent hover:text-accent-foreground",
        ghost: "bg-transparent text-muted-foreground hover:bg-foreground/6 hover:text-foreground",
        list: "bg-transparent text-muted-foreground hover:bg-accent hover:text-accent-foreground",
        danger:
          "[background:var(--destructive-gradient)] text-white shadow-[var(--shadow-control)] hover:brightness-[1.04] active:brightness-[0.98]",
      },
      size: {
        sm: "h-8 rounded-md px-2.5",
        md: "h-10 rounded-md px-3.5",
        lg: "h-11 rounded-md px-4 text-sm",
        icon: "size-8 rounded-md p-0",
      },
    },
    defaultVariants: { variant: "primary", size: "md" },
  },
);

type ButtonProps = ButtonHTMLAttributes<HTMLButtonElement> &
  VariantProps<typeof buttonVariants> & {
    asChild?: boolean;
    loading?: boolean;
  };

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(function Button(
  { asChild, className, variant, size, loading, disabled, children, ...props },
  ref,
) {
  const Component = asChild ? Slot : "button";
  return (
    <Component
      ref={ref}
      className={cn(buttonVariants({ variant, size }), className)}
      disabled={disabled || loading}
      {...props}
    >
      {loading ? <Spinner size={16} /> : null}
      {children}
    </Component>
  );
});

export { buttonVariants };
