import { useTranslation } from "react-i18next";

import { keysOf } from "./capture";

/** Key names in the UI Language where they differ ("Spacja", "Prawy Alt"). */
export function useKeyLabel(): (combination: string) => string {
  const { t } = useTranslation();
  const names: Partial<Record<string, string>> = t("keys", { returnObjects: true });
  return (combination: string) =>
    keysOf(combination)
      .map((key) => names[key] ?? key)
      .join(" + ");
}
