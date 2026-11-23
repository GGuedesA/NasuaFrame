import { describe, expect, it } from "vitest";

const art = import.meta.glob<string>("./assets/ornaments/*.svg", {
  query: "?raw",
  import: "default",
  eager: true,
});
const symbols = art["./assets/ornaments/symbols.svg"];
const pieces = Object.entries(art).filter(([file]) => !file.endsWith("/symbols.svg"));

describe("ornament assets", () => {
  it("finds every piece", () => {
    expect(pieces).toHaveLength(6);
  });

  for (const [file, svg] of pieces) {
    it(`${file} has no page-wide ids or styles`, () => {
      expect(svg).not.toMatch(/<style|<defs|\sid="/);
    });

    it(`${file} only uses shared symbols`, () => {
      for (const [, id] of svg.matchAll(/href="#([^"]+)"/g)) {
        expect(symbols).toContain(`id="${id}"`);
      }
    });
  }
});
