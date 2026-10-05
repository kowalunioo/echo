import "i18next";

import type { Translation } from "./en";

// Makes `t("…")` keys type-checked against the English resource.
declare module "i18next" {
  interface CustomTypeOptions {
    defaultNS: "translation";
    resources: { translation: Translation };
    returnNull: false;
  }
}
