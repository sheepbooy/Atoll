import "@testing-library/jest-dom/vitest";
import { cleanup } from "@testing-library/react";
import { afterEach, beforeEach } from "vitest";
import i18n from "../i18n";

beforeEach(async () => {
  window.localStorage.clear();
  await i18n.changeLanguage("en");
  document.documentElement.lang = "en";
});

afterEach(cleanup);
