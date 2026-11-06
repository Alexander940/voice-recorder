import { afterEach, describe, expect, it, vi } from "vitest";
import { defaultShortcut, isLinux, modeShortcut } from "./platform";

function withUserAgent(ua: string) {
  vi.spyOn(navigator, "userAgent", "get").mockReturnValue(ua);
}

describe("atajos por plataforma", () => {
  afterEach(() => vi.restoreAllMocks());

  it("macOS usa Option", () => {
    withUserAgent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15");
    expect(defaultShortcut()).toBe("Option+Space");
    expect(modeShortcut()).toBe("Ctrl+Option+M");
  });

  it("Windows usa Ctrl+Alt", () => {
    withUserAgent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36");
    expect(isLinux()).toBe(false);
    expect(defaultShortcut()).toBe("Ctrl+Alt+Space");
    expect(modeShortcut()).toBe("Ctrl+Alt+M");
  });

  it("Linux evita Ctrl+Alt (issue #38)", () => {
    withUserAgent("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/605.1.15 (KHTML, like Gecko)");
    expect(isLinux()).toBe(true);
    expect(defaultShortcut()).toBe("Ctrl+Shift+Space");
    expect(modeShortcut()).toBe("Ctrl+Shift+M");
  });
});
