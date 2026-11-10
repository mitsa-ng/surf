import { describe, expect, it } from "vitest";
import { displayAddress, hostOf, tabTitle } from "../lib/format";

describe("displayAddress", () => {
  it("strips the scheme", () => {
    expect(displayAddress("https://example.com/")).toBe("example.com");
    expect(displayAddress("http://example.com/path")).toBe("example.com/path");
  });

  it("keeps the query string", () => {
    expect(displayAddress("https://example.com/search?q=1")).toBe("example.com/search?q=1");
  });

  it("truncates long addresses in the middle", () => {
    const long = `https://example.com/${"a".repeat(200)}`;
    const shown = displayAddress(long);
    expect(shown.length).toBeLessThanOrEqual(96);
    expect(shown).toContain("…");
  });

  it("handles empty input", () => {
    expect(displayAddress(null)).toBe("");
  });
});

describe("hostOf", () => {
  it("extracts the host", () => {
    expect(hostOf("https://a.example.com/x")).toBe("a.example.com");
  });
  it("returns null for junk", () => {
    expect(hostOf("not a url")).toBeNull();
    expect(hostOf(null)).toBeNull();
  });
});

describe("tabTitle", () => {
  it("prefers the reported title", () => {
    expect(tabTitle({ id: 1, url: "https://x/", title: "X", loading: false })).toBe("X");
  });
  it("falls back to host then 新分頁", () => {
    expect(tabTitle({ id: 1, url: "https://x.example/", title: null, loading: false })).toBe("x.example");
    expect(tabTitle({ id: 1, url: null, title: null, loading: false })).toBe("新分頁");
  });
});
