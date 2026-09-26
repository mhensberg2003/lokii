import { describe, expect, it } from "vitest";
import { maskedKey, parsePort } from "./settings";

describe("parsePort", () => {
  it("reads an empty field as automatic", () => {
    expect(parsePort("")).toEqual({ port: null });
    expect(parsePort("  ")).toEqual({ port: null });
  });

  it("accepts ports from 1024 to 65535", () => {
    expect(parsePort("1024")).toEqual({ port: 1024 });
    expect(parsePort(" 51413 ")).toEqual({ port: 51413 });
    expect(parsePort("65535")).toEqual({ port: 65535 });
  });

  it("refuses other values", () => {
    for (const text of ["80", "65536", "12.5", "abc", "-2000"]) {
      expect(parsePort(text)).toHaveProperty("error");
    }
  });
});

describe("maskedKey", () => {
  it("shows only the last 4 characters", () => {
    expect(maskedKey("3456")).toBe("••••••••••••3456");
    expect(maskedKey(null)).toBe("••••••••••••");
  });
});
