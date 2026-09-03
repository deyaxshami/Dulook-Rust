import { createHash } from "node:crypto";
import { readFileSync, writeFileSync } from "node:fs";

const manifestUrl = new URL("../sources/hadith-api-selection.json", import.meta.url);
const outputUrl = new URL("../data/hadith-fallback.json", import.meta.url);
const manifest = JSON.parse(readFileSync(manifestUrl, "utf8"));
const checkOnly = process.argv.includes("--check");

const collections = {
  bukhari: {
    name: "Sahih al-Bukhari",
    slug: "bukhari",
    englishEdition: "eng-bukhari",
    arabicEdition: "ara-bukhari"
  },
  muslim: {
    name: "Sahih Muslim",
    slug: "muslim",
    englishEdition: "eng-muslim",
    arabicEdition: "ara-muslim"
  }
};

const fragmentPattern = /\b(?:same as (?:the )?(?:preceding|previous|above)|(?:rest of (?:the )?hadith|hadith|tradition|narration|report).{0,80}(?:same|similar)|same chain of transmitters|another chain of transmitters)\b/i;

function sourceUrl(path) {
  return `${manifest.source.repository}/raw/${manifest.source.commit}/${path}`;
}

async function fetchEdition([edition, details]) {
  const url = sourceUrl(details.path);
  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`Failed to fetch ${edition}: HTTP ${response.status}`);
  }

  const bytes = Buffer.from(await response.arrayBuffer());
  const digest = createHash("sha256").update(bytes).digest("hex");
  if (digest !== details.sha256) {
    throw new Error(`SHA-256 mismatch for ${edition}: expected ${details.sha256}, found ${digest}`);
  }

  return [edition, JSON.parse(bytes.toString("utf8"))];
}

function canonicalNumber(value) {
  const sourceNumber = String(value);
  const variant = /^(\d+)\.(\d+)$/.exec(sourceNumber);
  if (!variant) return sourceNumber;

  const suffixNumber = Number(variant[2]);
  if (suffixNumber < 1 || suffixNumber > 26) {
    throw new Error(`Unsupported canonical hadith suffix: ${sourceNumber}`);
  }
  return `${variant[1]}${String.fromCharCode(96 + suffixNumber)}`;
}

function assertSelectedText(record, collection, sourceHadithNumber) {
  const label = `${collection} source record ${sourceHadithNumber}`;
  const englishLength = record.english.length;
  const arabicLength = record.arabic.length;
  const policy = manifest.policy;

  if (englishLength < policy.minimumEnglishCharacters || englishLength > policy.maximumEnglishCharacters) {
    throw new Error(`${label} English length ${englishLength} is outside the selection policy`);
  }
  if (arabicLength < policy.minimumArabicCharacters || arabicLength > policy.maximumArabicCharacters) {
    throw new Error(`${label} Arabic length ${arabicLength} is outside the selection policy`);
  }
  if (fragmentPattern.test(record.english)) {
    throw new Error(`${label} contains a fragment/cross-reference phrase`);
  }
}

function buildRecords(editions) {
  const records = [];

  for (const [collection, config] of Object.entries(collections)) {
    const englishData = editions.get(config.englishEdition);
    const arabicData = editions.get(config.arabicEdition);
    const englishByNumber = new Map(englishData.hadiths.map((hadith) => [hadith.hadithnumber, hadith]));
    const arabicByNumber = new Map(arabicData.hadiths.map((hadith) => [hadith.hadithnumber, hadith]));

    for (const sourceHadithNumber of manifest.selection[collection]) {
      const english = englishByNumber.get(sourceHadithNumber);
      const arabic = arabicByNumber.get(sourceHadithNumber);
      if (!english || !arabic) {
        throw new Error(`Missing bilingual pair for ${collection} source record ${sourceHadithNumber}`);
      }
      if (JSON.stringify(english.reference) !== JSON.stringify(arabic.reference)) {
        throw new Error(`Reference mismatch for ${collection} source record ${sourceHadithNumber}`);
      }
      if (String(english.arabicnumber) !== String(arabic.arabicnumber)) {
        throw new Error(`Canonical number mismatch for ${collection} source record ${sourceHadithNumber}`);
      }

      const number = canonicalNumber(english.arabicnumber);
      const bookNumber = english.reference.book;
      const inBookHadithNumber = english.reference.hadith;
      const record = {
        id: `${collection}-${number}`,
        collection,
        collectionName: config.name,
        sourceHadithNumber,
        hadithNumber: number,
        bookNumber,
        bookName: englishData.metadata.sections[String(bookNumber)].trim(),
        inBookHadithNumber,
        reference: {
          canonical: `${config.name} ${number}`,
          inBook: `Book ${bookNumber}, Hadith ${inBookHadithNumber}`,
          url: `https://sunnah.com/${config.slug}:${number}`
        },
        arabic: arabic.text.trim(),
        english: english.text.trim()
      };

      assertSelectedText(record, collection, sourceHadithNumber);
      records.push(record);
    }
  }

  return records;
}

const editionEntries = Object.entries(manifest.source.editions);
const editions = new Map(await Promise.all(editionEntries.map(fetchEdition)));
const sourceEditions = Object.fromEntries(editionEntries.map(([edition, details]) => [edition, {
  url: sourceUrl(details.path),
  sha256: details.sha256,
  language: details.language,
  translator: details.translator
}]));

const output = {
  schemaVersion: 1,
  provenance: {
    source: manifest.source.name,
    repository: manifest.source.repository,
    commit: manifest.source.commit,
    commitUrl: `${manifest.source.repository}/commit/${manifest.source.commit}`,
    license: manifest.source.license,
    licenseUrl: `${manifest.source.repository}/blob/${manifest.source.commit}/LICENSE`,
    selectionManifest: "sources/hadith-api-selection.json",
    sourceEditions
  },
  selectionPolicy: manifest.policy,
  hadiths: buildRecords(editions)
};
const serialized = `${JSON.stringify(output, null, 2)}\n`;

if (checkOnly) {
  const current = readFileSync(outputUrl, "utf8");
  if (current !== serialized) {
    throw new Error("data/hadith-fallback.json is not reproducible from the pinned selection");
  }
  console.log(`Verified deterministic Hadith fallback (${output.hadiths.length} records).`);
} else {
  writeFileSync(outputUrl, serialized);
  console.log(`Wrote ${output.hadiths.length} records to data/hadith-fallback.json.`);
}
