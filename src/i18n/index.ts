import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import enCommon from "../locales/en/common.json";
import enSettings from "../locales/en/settings.json";
import enHooks from "../locales/en/hooks.json";
import enTokens from "../locales/en/tokens.json";
import enErrors from "../locales/en/errors.json";
import zhCommon from "../locales/zh-CN/common.json";
import zhSettings from "../locales/zh-CN/settings.json";
import zhHooks from "../locales/zh-CN/hooks.json";
import zhTokens from "../locales/zh-CN/tokens.json";
import zhErrors from "../locales/zh-CN/errors.json";

export const LANGUAGE_SETTING_KEY = "atoll.language";

export type AppLanguage = "en" | "zh-CN";

export const SUPPORTED_LANGUAGES: AppLanguage[] = ["en", "zh-CN"];

export function readLanguage(): AppLanguage {
  try {
    const stored = window.localStorage.getItem(LANGUAGE_SETTING_KEY);
    if (stored === "en" || stored === "zh-CN") {
      return stored;
    }
  } catch {
    // ignore storage failures
  }
  return "en";
}

export function writeLanguage(language: AppLanguage): void {
  try {
    window.localStorage.setItem(LANGUAGE_SETTING_KEY, language);
  } catch {
    // ignore storage failures
  }
}

export function resolveIntlLocale(language = i18n.language): string {
  return language === "zh-CN" ? "zh-CN" : "en-US";
}

export function syncDocumentLanguage(language: string): void {
  if (typeof document !== "undefined") {
    document.documentElement.lang = language;
  }
}

export async function changeAppLanguage(language: AppLanguage): Promise<void> {
  writeLanguage(language);
  await i18n.changeLanguage(language);
  syncDocumentLanguage(language);
}

void i18n.use(initReactI18next).init({
  resources: {
    en: {
      common: enCommon,
      settings: enSettings,
      hooks: enHooks,
      tokens: enTokens,
      errors: enErrors,
    },
    "zh-CN": {
      common: zhCommon,
      settings: zhSettings,
      hooks: zhHooks,
      tokens: zhTokens,
      errors: zhErrors,
    },
  },
  lng: readLanguage(),
  fallbackLng: "en",
  defaultNS: "common",
  interpolation: {
    escapeValue: false,
  },
});

syncDocumentLanguage(i18n.language);

export default i18n;
