import { Slot } from "@radix-ui/react-slot";
import { cva, type VariantProps } from "class-variance-authority";
import { forwardRef, type ButtonHTMLAttributes } from "react";

import { cn } from "@/lib/utils";
import { Spinner } from "./spinner";

const buttonVariants = cva(
  "inline-flex shrink-0 cursor-pointer items-center justify-center gap-2 whitespace-nowrap border-0 text-[13px] font-semibold transition-[color,background-color,transform,opacity,box-shadow] duration-150 outline-none focus-visible:ring-3 focus-visible:ring-ring/25 disabled:pointer-events-none disabled:opacity-50",
  {
    variants: {
      variant: {
        primary:
          "bg-primary text-primary-foreground shadow-[0_8px_18px_color-mix(in_srgb,var(--primary)_20%,transparent)] hover:bg-primary/92 hover:-translate-y-px",
        secondary:
          "bg-secondary text-secondary-foreground hover:bg-accent hover:text-accent-foreground",
        ghost: "bg-transparent text-muted-foreground hover:bg-foreground/6 hover:text-foreground",
        list: "bg-transparent text-muted-foreground hover:bg-accent hover:text-accent-foreground",
        danger: "bg-destructive text-white hover:bg-destructive/90",
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
