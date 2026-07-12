import { Eye, EyeOff } from "lucide-react";
import {
  forwardRef,
  useId,
  useState,
  type InputHTMLAttributes,
  type ReactNode,
} from "react";

import { cn } from "@/lib/utils";
import { Button } from "./button";

interface TextFieldProps extends Omit<InputHTMLAttributes<HTMLInputElement>, "size"> {
  label: string;
  hint?: string;
  error?: string;
  trailing?: ReactNode;
}

export const TextField = forwardRef<HTMLInputElement, TextFieldProps>(function TextField(
  { label, hint, error, className, id: providedId, trailing, ...props },
  ref,
) {
  const generatedId = useId();
  const id = providedId ?? generatedId;
  const descriptionId = hint || error ? `${id}-description` : undefined;
  return (
    <div className={cn("flex min-w-0 flex-1 flex-col gap-1.5", className)}>
      <label className="text-xs font-semibold text-foreground" htmlFor={id}>
        {label}
      </label>
      <div
        className={cn(
          "flex min-h-10 items-center overflow-hidden rounded-sm border border-input bg-background transition-[border-color,box-shadow] focus-within:border-ring focus-within:ring-3 focus-within:ring-ring/20",
          error && "border-destructive",
        )}
      >
        <input
          ref={ref}
          id={id}
          className="h-10 w-full min-w-0 border-0 bg-transparent px-3 text-sm text-foreground outline-none placeholder:text-muted-foreground/70"
          aria-invalid={Boolean(error)}
          aria-describedby={descriptionId}
          {...props}
        />
        {trailing}
      </div>
      {hint || error ? (
        <p
          id={descriptionId}
          className={cn("m-0 text-xs leading-relaxed text-muted-foreground", error && "text-destructive")}
        >
          {error ?? hint}
        </p>
      ) : null}
    </div>
  );
});

interface PasswordFieldProps extends TextFieldProps {
  showPasswordLabel: string;
  hidePasswordLabel: string;
}

export const PasswordField = forwardRef<HTMLInputElement, PasswordFieldProps>(
  function PasswordField({ showPasswordLabel, hidePasswordLabel, ...props }, ref) {
    const [visible, setVisible] = useState(false);
    return (
      <TextField
        {...props}
        ref={ref}
        type={visible ? "text" : "password"}
        autoComplete="current-password"
        trailing={
          <Button
            type="button"
            variant="ghost"
            size="icon"
            className="mr-1"
            aria-label={visible ? hidePasswordLabel : showPasswordLabel}
            onClick={() => setVisible((value) => !value)}
          >
            {visible ? <EyeOff size={17} /> : <Eye size={17} />}
          </Button>
        }
      />
    );
  },
);

