import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

import { validateHadithData } from "../scripts/validate-data.mjs";

const data = JSON.parse(readFileSync(new URL("../data/hadith-fallback.json", import.meta.url), "utf8"));
const manifest = JSON.parse(readFileSync(new URL("../sources/hadith-api-selection.json", import.meta.url), "utf8"));
const records = data.hadiths;

test("contains exactly 100 logical records split 50/50", () => {
  assert.equal(records.length, 100);
  assert.equal(records.filter(({ collection }) => collection === "bukhari").length, 50);
  assert.equal(records.filter(({ collection }) => collection === "muslim").length, 50);
});

test("every record has a nonempty, panel-sized Arabic/English pair", () => {
  for (const record of records) {
    assert.match(record.arabic, /[\u0600-\u06ff]/u, record.id);
    assert.match(record.english, /[A-Za-z]/, record.id);
    assert.ok(record.arabic.length >= manifest.policy.minimumArabicCharacters, record.id);
    assert.ok(record.arabic.length <= manifest.policy.maximumArabicCharacters, record.id);
    assert.ok(record.english.length >= manifest.policy.minimumEnglishCharacters, record.id);
    assert.ok(record.english.length <= manifest.policy.maximumEnglishCharacters, record.id);
    assert.doesNotMatch(record.english, /same as (?:the )?(?:preceding|previous|above)|same chain of transmitters|another chain of transmitters|rest of (?:the )?hadith/i, record.id);
  }
});

test("IDs and bilingual texts are unique", () => {
  const normalize = (text) => text.normalize("NFKC").trim().replace(/\s+/g, " ").toLowerCase();
  assert.equal(new Set(records.map(({ id }) => id)).size, 100);
  assert.equal(new Set(records.map(({ english }) => normalize(english))).size, 100);
  assert.equal(new Set(records.map(({ arabic }) => normalize(arabic))).size, 100);
});

test("canonical and in-book references are useful and internally consistent", () => {
  for (const record of records) {
    assert.match(record.hadithNumber, /^\d+[a-z]?$/, record.id);
    assert.equal(record.reference.canonical, `${record.collectionName} ${record.hadithNumber}`);
    assert.equal(record.reference.inBook, `Book ${record.bookNumber}, Hadith ${record.inBookHadithNumber}`);
    assert.equal(record.reference.url, `https://sunnah.com/${record.collection}:${record.hadithNumber}`);
    assert.ok(record.bookName.length > 0, record.id);
  }
});

test("provenance is immutable and matches the reviewed source selection", () => {
  assert.match(data.provenance.commit, /^[0-9a-f]{40}$/);
  assert.equal(data.provenance.commit, manifest.source.commit);
  assert.equal(data.provenance.repository, manifest.source.repository);
  assert.equal(data.provenance.selectionManifest, "sources/hadith-api-selection.json");

  for (const [edition, details] of Object.entries(data.provenance.sourceEditions)) {
    assert.match(details.url, new RegExp(`/${manifest.source.commit}/`), edition);
    assert.equal(details.sha256, manifest.source.editions[edition].sha256);
  }

  for (const collection of ["bukhari", "muslim"]) {
    assert.deepEqual(
      records.filter((record) => record.collection === collection).map((record) => record.sourceHadithNumber),
      manifest.selection[collection]
    );
  }
});

test("the shared runtime validator accepts the generated artifact", () => {
  assert.deepEqual(validateHadithData(data, manifest), {
    total: 100,
    collectionCounts: { bukhari: 50, muslim: 50 }
  });
});
