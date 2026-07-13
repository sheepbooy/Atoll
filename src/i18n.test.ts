import { beforeEach, describe, expect, it } from "vitest";
import i18n, {
  LANGUAGE_SETTING_KEY,
  readLanguage,
  resolveIntlLocale,
  writeLanguage,
} from "./i18n";

describe("i18n", () => {
  beforeEach(async () => {
    window.localStorage.clear();
    await i18n.changeLanguage("en");
    document.documentElement.lang = "en";
  });

  it("defaults to English when no preference is stored", () => {
    expect(readLanguage()).toBe("en");
  });

  it("persists language preference in localStorage", () => {
    writeLanguage("zh-CN");
    expect(window.localStorage.getItem(LANGUAGE_SETTING_KEY)).toBe("zh-CN");
    expect(readLanguage()).toBe("zh-CN");
  });

  it("maps locale codes for Intl formatting", () => {
    expect(resolveIntlLocale("en")).toBe("en-US");
    expect(resolveIntlLocale("zh-CN")).toBe("zh-CN");
  });

  it("translates a settings label in Chinese", async () => {
    await i18n.changeLanguage("zh-CN");
    expect(i18n.t("display.languageLabel", { ns: "settings" })).toBe("语言");
  });
});
