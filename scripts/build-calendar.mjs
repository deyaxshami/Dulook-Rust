import { readFileSync, writeFileSync } from "node:fs";

const source = new URL("../SalatCalender2026_Nordhorn.csv", import.meta.url);
const destination = new URL("../SalatCalender2026_Nordhorn.js", import.meta.url);
const csv = readFileSync(source, "utf8");
const rows = csv.trim().split(/\r?\n/);

if (rows.length !== 366) {
  throw new Error(`Expected 365 calendar entries, found ${rows.length - 1}`);
}

writeFileSync(destination, `window.DULOOK_PRAYER_CSV = ${JSON.stringify(csv)};\n`);
