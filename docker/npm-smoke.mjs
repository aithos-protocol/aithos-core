// npm smoke for @aithos/core (plan §K): import the locally packed WASM
// package and cross-check genesis_pubkeys against the frozen A1 vector.
// Run from the repo root AFTER `npm install <tarball>` in a scratch dir:
//   node docker/npm-smoke.mjs <path-to-a1-genesis.json>
import { readFileSync } from "node:fs";
import { genesis_pubkeys } from "@aithos/core";

const vecPath = process.argv[2] ?? "vectors/a1-genesis.json";
const vec = JSON.parse(readFileSync(vecPath, "utf8"));
if (!vec.seed_hex) throw new Error("A1 vector: seed_hex not found");
const seed = Uint8Array.from(
  vec.seed_hex.match(/../g).map((b) => parseInt(b, 16)),
);

const got = JSON.parse(genesis_pubkeys(seed));
for (const k of ["root_sign_pub", "content_sign_pub", "owner_kex_pub"]) {
  const want = vec[`${k}_hex`];
  if (!want) throw new Error(`A1 vector: ${k}_hex not found`);
  if (got[k] !== want) throw new Error(`${k}: got ${got[k]}, want ${want}`);
}
console.log("npm smoke OK — genesis_pubkeys matches the A1 vector byte for byte");
