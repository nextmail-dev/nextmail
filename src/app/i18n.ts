import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import enUS from "../locales/en-US/common.json";
import zhCN from "../locales/zh-CN/common.json";

void i18n.use(initReactI18next).init({
  resources: {
    "zh-CN": { common: zhCN },
    "en-US": { common: enUS },
  },
  lng: "zh-CN",
  fallbackLng: "en-US",
  defaultNS: "common",
  interpolation: { escapeValue: false },
});

export default i18n;

