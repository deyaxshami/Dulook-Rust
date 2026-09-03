import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";

const defaultDataUrl = new URL("../data/hadith-fallback.json", import.meta.url);
const defaultManifestUrl = new URL("../sources/hadith-api-selection.json", import.meta.url);
const fragmentPattern = /\b(?:same as (?:the )?(?:preceding|previous|above)|(?:rest of (?:the )?hadith|hadith|tradition|narration|report).{0,80}(?:same|similar)|same chain of transmitters|another chain of transmitters)\b/i;

function invariant(condition, message) {
  if (!condition) throw new Error(message);
}

function normalizedText(text) {
  return text.normalize("NFKC").trim().replace(/\s+/g, " ").toLowerCase();
}

export function validateHadithData(data, manifest) {
  invariant(data.schemaVersion === 1, "Hadith data must use schema version 1");
  invariant(Array.isArray(data.hadiths), "Hadith data must contain a hadiths array");
  invariant(data.hadiths.length === 100, `Expected 100 Hadith records, found ${data.hadiths.length}`);

  const expectedCommit = manifest.source.commit;
  invariant(/^[0-9a-f]{40}$/.test(expectedCommit), "Source commit must be a full immutable Git SHA");
  invariant(data.provenance?.source === "hadith-api", "Missing hadith-api provenance");
  invariant(data.provenance.repository === manifest.source.repository, "Provenance repository does not match the manifest");
  invariant(data.provenance.commit === expectedCommit, "Provenance commit does not match the manifest");
  invariant(data.provenance.commitUrl.endsWith(`/commit/${expectedCommit}`), "Provenance commit URL is not pinned");
  invariant(data.provenance.license === manifest.source.license, "Provenance license does not match the manifest");
  invariant(data.provenance.selectionManifest === "sources/hadith-api-selection.json", "Missing selection manifest provenance");

  for (const [edition, source] of Object.entries(manifest.source.editions)) {
    const provenance = data.provenance.sourceEditions?.[edition];
    invariant(provenance?.sha256 === source.sha256, `${edition} SHA-256 provenance is missing or incorrect`);
    invariant(provenance.url.includes(`/${expectedCommit}/`), `${edition} URL is not pinned to the source commit`);
  }

  const ids = new Set();
  const englishTexts = new Set();
  const arabicTexts = new Set();
  const collectionCounts = { bukhari: 0, muslim: 0 };
  const collectionBooks = { bukhari: new Set(), muslim: new Set() };
  const sourceNumbers = { bukhari: [], muslim: [] };

  for (const record of data.hadiths) {
    const label = record.id || "record without ID";
    invariant(record.collection === "bukhari" || record.collection === "muslim", `${label} has an unsupported collection`);
    invariant(typeof record.id === "string" && record.id.length > 0, "Every record needs an ID");
    invariant(!ids.has(record.id), `Duplicate Hadith ID: ${record.id}`);
    ids.add(record.id);

    invariant(typeof record.arabic === "string" && record.arabic.trim().length > 0, `${label} has empty Arabic text`);
    invariant(typeof record.english === "string" && record.english.trim().length > 0, `${label} has empty English text`);
    invariant(/[\u0600-\u06ff]/u.test(record.arabic), `${label} Arabic text has no Arabic characters`);
    invariant(/[A-Za-z]/.test(record.english), `${label} English text has no Latin characters`);
    invariant(record.arabic.length >= manifest.policy.minimumArabicCharacters, `${label} Arabic text is too short`);
    invariant(record.arabic.length <= manifest.policy.maximumArabicCharacters, `${label} Arabic text is too long for the panel`);
    invariant(record.english.length >= manifest.policy.minimumEnglishCharacters, `${label} English text is too short`);
    invariant(record.english.length <= manifest.policy.maximumEnglishCharacters, `${label} English text is too long for the panel`);
    invariant(!fragmentPattern.test(record.english), `${label} is an obvious fragment or cross-reference`);

    const normalizedEnglish = normalizedText(record.english);
    const normalizedArabic = normalizedText(record.arabic);
    invariant(!englishTexts.has(normalizedEnglish), `${label} duplicates English text`);
    invariant(!arabicTexts.has(normalizedArabic), `${label} duplicates Arabic text`);
    englishTexts.add(normalizedEnglish);
    arabicTexts.add(normalizedArabic);

    invariant(Number.isInteger(record.sourceHadithNumber) && record.sourceHadithNumber > 0, `${label} has an invalid source Hadith number`);
    invariant(typeof record.hadithNumber === "string" && /^\d+[a-z]?$/.test(record.hadithNumber), `${label} has an invalid canonical Hadith number`);
    invariant(Number.isInteger(record.bookNumber) && record.bookNumber > 0, `${label} has an invalid book number`);
    invariant(typeof record.bookName === "string" && record.bookName.trim().length > 0, `${label} has no book name`);
    invariant(Number.isInteger(record.inBookHadithNumber) && record.inBookHadithNumber > 0, `${label} has an invalid in-book Hadith number`);
    invariant(record.reference?.canonical === `${record.collectionName} ${record.hadithNumber}`, `${label} has an invalid canonical reference`);
    invariant(record.reference.inBook === `Book ${record.bookNumber}, Hadith ${record.inBookHadithNumber}`, `${label} has an invalid in-book reference`);
    invariant(record.reference.url === `https://sunnah.com/${record.collection}:${record.hadithNumber}`, `${label} has an invalid canonical URL`);

    collectionCounts[record.collection] += 1;
    collectionBooks[record.collection].add(record.bookNumber);
    sourceNumbers[record.collection].push(record.sourceHadithNumber);
  }

  for (const collection of ["bukhari", "muslim"]) {
    invariant(collectionCounts[collection] === 50, `Expected 50 ${collection} records, found ${collectionCounts[collection]}`);
    invariant(collectionBooks[collection].size === 50, `${collection} selection must contain one record per selected book`);
    invariant(JSON.stringify(sourceNumbers[collection]) === JSON.stringify(manifest.selection[collection]), `${collection} records do not match the reviewed selection manifest`);
  }

  return { total: data.hadiths.length, collectionCounts };
}

export function readAndValidateHadithData(dataUrl = defaultDataUrl, manifestUrl = defaultManifestUrl) {
  const data = JSON.parse(readFileSync(dataUrl, "utf8"));
  const manifest = JSON.parse(readFileSync(manifestUrl, "utf8"));
  return validateHadithData(data, manifest);
}

const invokedPath = process.argv[1] ? pathToFileURL(resolve(process.argv[1])).href : "";
if (invokedPath === import.meta.url) {
  const result = readAndValidateHadithData();
  console.log(`Validated ${result.total} bilingual Hadith records (50 Bukhari, 50 Muslim).`);
}
