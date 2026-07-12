import { Mail } from "lucide-react";
import { useTranslation } from "react-i18next";

import { IconTile } from "@/components/ui/icon-tile";
import { Modal } from "@/components/ui/dialog";
import { Stack } from "@/components/ui/layout";
import { Heading, Text } from "@/components/ui/typography";

interface AboutDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  version: string;
}

export function AboutDialog({ open, onOpenChange, version }: AboutDialogProps) {
  const { t } = useTranslation();
  return (
    <Modal
      open={open}
      onOpenChange={onOpenChange}
      title={t("about.title")}
      closeLabel={t("common.close")}
    >
      <Stack className="mt-5 items-start" gap="md">
        <IconTile large>
          <Mail size={24} />
        </IconTile>
        <Heading level={3}>NextMail</Heading>
        <Text>{t("about.version", { version })}</Text>
        <Text>{t("about.description")}</Text>
      </Stack>
    </Modal>
  );
}
