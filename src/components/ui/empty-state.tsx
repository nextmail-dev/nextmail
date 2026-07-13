import type { ReactNode } from "react";

import { Stack } from "./layout";
import { Heading, Text } from "./typography";

interface EmptyStateProps {
  icon: ReactNode;
  title: string;
  description?: string;
  action?: ReactNode;
  className?: string;
}

export function EmptyState({ icon, title, description, action, className }: EmptyStateProps) {
  return (
    <Stack className={className ?? "m-auto max-w-xs items-center p-7 text-center"} gap="sm">
      <span className="flex size-11 items-center justify-center rounded-lg bg-primary/10 text-primary">
        {icon}
      </span>
      <Heading level={3}>{title}</Heading>
      {description ? <Text>{description}</Text> : null}
      {action}
    </Stack>
  );
}
