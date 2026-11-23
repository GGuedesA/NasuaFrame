import { expect, test } from "vitest";
import { deal } from "#lib/deal.ts";

test("buy below the limit", () => {
  expect(deal("buy", 100, 75)).toEqual({ depth: 0.25, label: "-25%" });
});

test("right at the limit", () => {
  expect(deal("buy", 100, 100)).toEqual({ depth: 0, label: "0%" });
});

test("sell above the limit", () => {
  expect(deal("sell", 100, 200)).toEqual({ depth: 0.5, label: "+100%" });
});

test("zero limit", () => {
  expect(deal("buy", 0, 0)).toEqual({ depth: 0, label: "" });
  expect(deal("sell", 0, 5)).toEqual({ depth: 1, label: "" });
});
