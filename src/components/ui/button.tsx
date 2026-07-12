import { Slot } from "@radix-ui/react-slot";
import { cva, type VariantProps } from "class-variance-authority";
import { forwardRef, type ButtonHTMLAttributes } from "react";

import { cn } from "@/lib/utils";
import { Spinner } from "./spinner";

const buttonVariants = cva(
  "inline-flex shrink-0 cursor-pointer items-center justify-center gap-2 whitespace-nowrap border text-[13px] font-semibold transition-[color,background-color,border-color,transform,opacity] duration-150 outline-none focus-visible:ring-3 focus-visible:ring-ring/25 disabled:pointer-events-none disabled:opacity-50",
  {
    variants: {
      variant: {
        primary:
          "border-primary bg-primary text-primary-foreground shadow-sm hover:bg-primary/90 hover:-translate-y-px",
        secondary:
          "border-border bg-background text-foreground hover:bg-accent hover:text-accent-foreground",
        ghost: "border-0 bg-transparent text-muted-foreground hover:bg-accent hover:text-accent-foreground",
        list: "border-x-0 border-t-0 border-b border-border bg-transparent text-muted-foreground hover:bg-accent hover:text-accent-foreground",
        danger: "border-destructive bg-destructive text-white hover:bg-destructive/90",
      },
      size: {
        sm: "h-8 rounded-sm px-2.5",
        md: "h-10 rounded-sm px-3.5",
        lg: "h-11 rounded-sm px-4 text-sm",
        icon: "size-8 rounded-sm p-0",
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
