import { LoaderCircle } from "lucide-react";

export function Spinner({ size = 24 }: { size?: number }) {
  return <LoaderCircle className="animate-spin" size={size} aria-hidden="true" />;
}

