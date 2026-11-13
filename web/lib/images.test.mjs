import test from "node:test";
import assert from "node:assert/strict";
import { smallVariant, shrinkPlan, MAX_SIDE } from "./images.mjs";

const id = "0b7f3c1e-2d4a-4f5b-9c6d-7e8f9a0b1c2d";

test("the card copy exists only for pictures the API stored as WebP", () => {
  assert.equal(smallVariant(`https://x.test/uploads/${id}.webp`), `https://x.test/uploads/${id}-480.webp`);
  assert.equal(smallVariant(`/uploads/${id}.webp`), `/uploads/${id}-480.webp`);
  assert.equal(smallVariant(`https://x.test/uploads/${id}.jpg`), null, "an older upload has no copy");
  assert.equal(smallVariant(`https://x.test/uploads/${id}-480.webp`), null, "the copy has no copy");
  assert.equal(smallVariant(`https://x.test/uploads/${id}.webp?x=1`), null);
  assert.equal(smallVariant("https://cdn.example/poster.webp"), null, "external links stay as they are");
  assert.equal(smallVariant(null), null);
  assert.equal(smallVariant(undefined), null);
});

test("big photos are shrunk to 1600 px before upload; small ones go as they are", () => {
  assert.deepEqual(shrinkPlan({ size: 6_000_000, type: "image/jpeg", width: 4032, height: 3024 }), { width: MAX_SIDE, height: 1200 });
  assert.deepEqual(shrinkPlan({ size: 3_000_000, type: "image/png", width: 1000, height: 3000 }), { width: 533, height: 1600 });
  assert.deepEqual(shrinkPlan({ size: 2_000_000, type: "image/jpeg", width: 1200, height: 800 }), { width: 1200, height: 800 }, "heavy but small: re-encode only");
  assert.equal(shrinkPlan({ size: 400_000, type: "image/jpeg", width: 1200, height: 800 }), null);
  assert.equal(shrinkPlan({ size: 9_000_000, type: "image/heic", width: 4032, height: 3024 }), null, "unknown types go to the API, which explains");
  assert.equal(shrinkPlan({ size: 9_000_000, type: "image/jpeg", width: 0, height: 0 }), null);
});
