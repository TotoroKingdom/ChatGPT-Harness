import "@testing-library/jest-dom/vitest";
import { beforeEach } from "vitest";
import { initializeLanguage } from "@/i18n";

beforeEach(async () => { await initializeLanguage("en"); });
