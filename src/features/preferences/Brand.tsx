import { Mail } from "lucide-react";
import { useTranslation } from "react-i18next";

import { IconTile } from "@/components/ui/icon-tile";
import { Inline, Stack } from "@/components/ui/layout";
import { Text } from "@/components/ui/typography";

export function Brand() {
  const { t } = useTranslation();
  return (
    <Inline className="gap-3">
      <IconTile>
        <Mail size={20} />
      </IconTile>
      <Stack gap="xs">
        <strong className="text-base tracking-tight">{t("app.name")}</strong>
        <Text className="max-w-52 text-[length:var(--ui-font-caption)] leading-snug text-muted-foreground/80">
          {t("app.tagline")}
        </Text>
      </Stack>
    </Inline>
  );
}
