//! Offline Hadith collection: validation and deterministic rebuild
//! (port of the original `scripts/validate-data.mjs` and `scripts/build-hadith-fallback.mjs`).

use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

use regex::Regex;
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use unicode_normalization::UnicodeNormalization;

use crate::jsutil::{js_string, js_trim, utf16_len};

pub const SELECTION_MANIFEST: &str = "sources/hadith-api-selection.json";

static FRAGMENT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)(?-u:\b)(?:same as (?:the )?(?:preceding|previous|above)|(?:rest of (?:the )?hadith|hadith|tradition|narration|report).{0,80}(?:same|similar)|same chain of transmitters|another chain of transmitters)(?-u:\b)",
    )
    .unwrap()
});
static ARABIC: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[\x{0600}-\x{06FF}]").unwrap());
static LATIN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[A-Za-z]").unwrap());
static CANONICAL_NUMBER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[0-9]+[a-z]?$").unwrap());
static COMMIT_SHA: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[0-9a-f]{40}$").unwrap());
static VARIANT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^([0-9]+)\.([0-9]+)$").unwrap());

const COLLECTIONS: [(&str, &str, &str, &str); 2] = [
    ("bukhari", "Sahih al-Bukhari", "eng-bukhari", "ara-bukhari"),
    ("muslim", "Sahih Muslim", "eng-muslim", "ara-muslim"),
];

#[derive(Debug, PartialEq, Eq)]
pub struct HadithDataSummary {
    pub total: usize,
    pub bukhari: usize,
    pub muslim: usize,
}

/// True when `text` contains an obvious fragment or cross-reference phrase.
pub fn is_fragment(text: &str) -> bool {
    FRAGMENT.is_match(text)
}

fn invariant(condition: bool, message: impl FnOnce() -> String) -> Result<(), String> {
    if condition { Ok(()) } else { Err(message()) }
}

fn normalized_text(text: &str) -> String {
    let nfkc: String = text.nfkc().collect();
    js_trim(&nfkc).split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

/// JS template-literal interpolation: missing values render as `undefined`.
fn interp(value: Option<&Value>) -> String {
    value.map_or_else(|| "undefined".to_owned(), js_string)
}

fn as_str(value: Option<&Value>) -> Option<&str> {
    value.and_then(Value::as_str)
}

fn is_positive_integer(value: Option<&Value>) -> bool {
    value.and_then(Value::as_f64).is_some_and(|n| n.fract() == 0.0 && n > 0.0)
}

fn policy_number(manifest: &Value, key: &str) -> f64 {
    manifest["policy"][key].as_f64().unwrap_or(f64::NAN)
}

pub fn validate_hadith_data(data: &Value, manifest: &Value) -> Result<HadithDataSummary, String> {
    invariant(data["schemaVersion"].as_f64() == Some(1.0), || "Hadith data must use schema version 1".into())?;
    let records = data["hadiths"].as_array().ok_or("Hadith data must contain a hadiths array")?;
    invariant(records.len() == 100, || format!("Expected 100 Hadith records, found {}", records.len()))?;

    let expected_commit = manifest["source"]["commit"].as_str().unwrap_or("");
    invariant(COMMIT_SHA.is_match(expected_commit), || "Source commit must be a full immutable Git SHA".into())?;
    let provenance = &data["provenance"];
    invariant(provenance["source"].as_str() == Some("hadith-api"), || "Missing hadith-api provenance".into())?;
    invariant(provenance["repository"] == manifest["source"]["repository"], || {
        "Provenance repository does not match the manifest".into()
    })?;
    invariant(provenance["commit"].as_str() == Some(expected_commit), || {
        "Provenance commit does not match the manifest".into()
    })?;
    invariant(
        provenance["commitUrl"].as_str().is_some_and(|url| url.ends_with(&format!("/commit/{expected_commit}"))),
        || "Provenance commit URL is not pinned".into(),
    )?;
    invariant(provenance["license"] == manifest["source"]["license"], || {
        "Provenance license does not match the manifest".into()
    })?;
    invariant(provenance["selectionManifest"].as_str() == Some(SELECTION_MANIFEST), || {
        "Missing selection manifest provenance".into()
    })?;

    let editions = manifest["source"]["editions"].as_object().cloned().unwrap_or_default();
    for (edition, source) in &editions {
        let edition_provenance = &provenance["sourceEditions"][edition];
        invariant(
            !edition_provenance.is_null() && edition_provenance["sha256"] == source["sha256"],
            || format!("{edition} SHA-256 provenance is missing or incorrect"),
        )?;
        invariant(
            edition_provenance["url"].as_str().is_some_and(|url| url.contains(&format!("/{expected_commit}/"))),
            || format!("{edition} URL is not pinned to the source commit"),
        )?;
    }

    let min_arabic = policy_number(manifest, "minimumArabicCharacters");
    let max_arabic = policy_number(manifest, "maximumArabicCharacters");
    let min_english = policy_number(manifest, "minimumEnglishCharacters");
    let max_english = policy_number(manifest, "maximumEnglishCharacters");

    let mut ids = HashSet::new();
    let mut english_texts = HashSet::new();
    let mut arabic_texts = HashSet::new();
    let mut counts: HashMap<&str, usize> = HashMap::from([("bukhari", 0), ("muslim", 0)]);
    let mut books: HashMap<&str, HashSet<String>> = HashMap::from([("bukhari", HashSet::new()), ("muslim", HashSet::new())]);
    let mut source_numbers: HashMap<&str, Vec<Value>> = HashMap::from([("bukhari", Vec::new()), ("muslim", Vec::new())]);

    for record in records {
        let id = as_str(record.get("id")).unwrap_or("");
        let label = if id.is_empty() { "record without ID" } else { id };
        let collection = match as_str(record.get("collection")) {
            Some("bukhari") => "bukhari",
            Some("muslim") => "muslim",
            _ => return Err(format!("{label} has an unsupported collection")),
        };
        invariant(!id.is_empty(), || "Every record needs an ID".into())?;
        invariant(ids.insert(id.to_owned()), || format!("Duplicate Hadith ID: {id}"))?;

        let arabic = as_str(record.get("arabic"));
        let english = as_str(record.get("english"));
        invariant(arabic.is_some_and(|t| !js_trim(t).is_empty()), || format!("{label} has empty Arabic text"))?;
        invariant(english.is_some_and(|t| !js_trim(t).is_empty()), || format!("{label} has empty English text"))?;
        let (arabic, english) = (arabic.unwrap_or(""), english.unwrap_or(""));
        invariant(ARABIC.is_match(arabic), || format!("{label} Arabic text has no Arabic characters"))?;
        invariant(LATIN.is_match(english), || format!("{label} English text has no Latin characters"))?;
        let (arabic_len, english_len) = (utf16_len(arabic) as f64, utf16_len(english) as f64);
        invariant(arabic_len >= min_arabic, || format!("{label} Arabic text is too short"))?;
        invariant(arabic_len <= max_arabic, || format!("{label} Arabic text is too long for the panel"))?;
        invariant(english_len >= min_english, || format!("{label} English text is too short"))?;
        invariant(english_len <= max_english, || format!("{label} English text is too long for the panel"))?;
        invariant(!is_fragment(english), || format!("{label} is an obvious fragment or cross-reference"))?;

        invariant(english_texts.insert(normalized_text(english)), || format!("{label} duplicates English text"))?;
        invariant(arabic_texts.insert(normalized_text(arabic)), || format!("{label} duplicates Arabic text"))?;

        invariant(is_positive_integer(record.get("sourceHadithNumber")), || {
            format!("{label} has an invalid source Hadith number")
        })?;
        invariant(as_str(record.get("hadithNumber")).is_some_and(|n| CANONICAL_NUMBER.is_match(n)), || {
            format!("{label} has an invalid canonical Hadith number")
        })?;
        invariant(is_positive_integer(record.get("bookNumber")), || format!("{label} has an invalid book number"))?;
        invariant(as_str(record.get("bookName")).is_some_and(|n| !js_trim(n).is_empty()), || {
            format!("{label} has no book name")
        })?;
        invariant(is_positive_integer(record.get("inBookHadithNumber")), || {
            format!("{label} has an invalid in-book Hadith number")
        })?;
        let reference = record.get("reference");
        let hadith_number = interp(record.get("hadithNumber"));
        let book_number = interp(record.get("bookNumber"));
        let in_book_number = interp(record.get("inBookHadithNumber"));
        invariant(
            as_str(reference.and_then(|r| r.get("canonical")))
                == Some(&format!("{} {hadith_number}", interp(record.get("collectionName")))),
            || format!("{label} has an invalid canonical reference"),
        )?;
        invariant(
            as_str(reference.and_then(|r| r.get("inBook"))) == Some(&format!("Book {book_number}, Hadith {in_book_number}")),
            || format!("{label} has an invalid in-book reference"),
        )?;
        invariant(
            as_str(reference.and_then(|r| r.get("url"))) == Some(&format!("https://sunnah.com/{collection}:{hadith_number}")),
            || format!("{label} has an invalid canonical URL"),
        )?;

        *counts.get_mut(collection).unwrap() += 1;
        books.get_mut(collection).unwrap().insert(book_number);
        source_numbers.get_mut(collection).unwrap().push(record["sourceHadithNumber"].clone());
    }

    for collection in ["bukhari", "muslim"] {
        let count = counts[collection];
        invariant(count == 50, || format!("Expected 50 {collection} records, found {count}"))?;
        invariant(books[collection].len() == 50, || {
            format!("{collection} selection must contain one record per selected book")
        })?;
        let actual = serde_json::to_string(&source_numbers[collection]).unwrap_or_default();
        let expected = serde_json::to_string(&manifest["selection"][collection]).unwrap_or_default();
        invariant(actual == expected, || format!("{collection} records do not match the reviewed selection manifest"))?;
    }

    Ok(HadithDataSummary { total: records.len(), bukhari: counts["bukhari"], muslim: counts["muslim"] })
}

fn canonical_number(value: &Value) -> Result<String, String> {
    let source_number = js_string(value);
    let Some(variant) = VARIANT.captures(&source_number) else { return Ok(source_number) };
    let suffix: u32 = variant[2].parse().unwrap_or(0);
    if !(1..=26).contains(&suffix) {
        return Err(format!("Unsupported canonical hadith suffix: {source_number}"));
    }
    Ok(format!("{}{}", &variant[1], char::from_u32(96 + suffix).unwrap()))
}

fn assert_selected_text(manifest: &Value, record: &Map<String, Value>, collection: &str, source_number: &Value) -> Result<(), String> {
    let label = format!("{collection} source record {}", js_string(source_number));
    let english = record["english"].as_str().unwrap_or("");
    let arabic = record["arabic"].as_str().unwrap_or("");
    let (english_len, arabic_len) = (utf16_len(english), utf16_len(arabic));
    let (el, al) = (english_len as f64, arabic_len as f64);
    if el < policy_number(manifest, "minimumEnglishCharacters") || el > policy_number(manifest, "maximumEnglishCharacters") {
        return Err(format!("{label} English length {english_len} is outside the selection policy"));
    }
    if al < policy_number(manifest, "minimumArabicCharacters") || al > policy_number(manifest, "maximumArabicCharacters") {
        return Err(format!("{label} Arabic length {arabic_len} is outside the selection policy"));
    }
    if is_fragment(english) {
        return Err(format!("{label} contains a fragment/cross-reference phrase"));
    }
    Ok(())
}

fn by_number(edition: &Value) -> HashMap<String, &Value> {
    edition["hadiths"]
        .as_array()
        .map(|hadiths| hadiths.iter().map(|h| (js_string(&h["hadithnumber"]), h)).collect())
        .unwrap_or_default()
}

fn build_records(manifest: &Value, editions: &HashMap<String, Value>) -> Result<Vec<Value>, String> {
    let mut records = Vec::new();
    for (collection, name, english_edition, arabic_edition) in COLLECTIONS {
        let english_data = editions.get(english_edition).ok_or_else(|| format!("Missing edition {english_edition}"))?;
        let arabic_data = editions.get(arabic_edition).ok_or_else(|| format!("Missing edition {arabic_edition}"))?;
        let english_by_number = by_number(english_data);
        let arabic_by_number = by_number(arabic_data);

        for source_number in manifest["selection"][collection].as_array().into_iter().flatten() {
            let key = js_string(source_number);
            let label = format!("{collection} source record {key}");
            let (Some(english), Some(arabic)) = (english_by_number.get(&key), arabic_by_number.get(&key)) else {
                return Err(format!("Missing bilingual pair for {label}"));
            };
            if serde_json::to_string(&english["reference"]).ok() != serde_json::to_string(&arabic["reference"]).ok() {
                return Err(format!("Reference mismatch for {label}"));
            }
            if interp(english.get("arabicnumber")) != interp(arabic.get("arabicnumber")) {
                return Err(format!("Canonical number mismatch for {label}"));
            }

            let number = canonical_number(&english["arabicnumber"])?;
            let book_number = english["reference"]["book"].clone();
            let in_book_number = english["reference"]["hadith"].clone();
            let book_name = english_data["metadata"]["sections"][js_string(&book_number)]
                .as_str()
                .map(|s| js_trim(s).to_owned())
                .ok_or_else(|| format!("Missing book name for {label}"))?;
            let mut record = Map::new();
            record.insert("id".into(), json!(format!("{collection}-{number}")));
            record.insert("collection".into(), json!(collection));
            record.insert("collectionName".into(), json!(name));
            record.insert("sourceHadithNumber".into(), source_number.clone());
            record.insert("hadithNumber".into(), json!(number));
            record.insert("bookNumber".into(), book_number.clone());
            record.insert("bookName".into(), json!(book_name));
            record.insert("inBookHadithNumber".into(), in_book_number.clone());
            let mut reference = Map::new();
            reference.insert("canonical".into(), json!(format!("{name} {number}")));
            reference.insert(
                "inBook".into(),
                json!(format!("Book {}, Hadith {}", js_string(&book_number), js_string(&in_book_number))),
            );
            reference.insert("url".into(), json!(format!("https://sunnah.com/{collection}:{number}")));
            record.insert("reference".into(), Value::Object(reference));
            record.insert("arabic".into(), json!(js_trim(arabic["text"].as_str().unwrap_or(""))));
            record.insert("english".into(), json!(js_trim(english["text"].as_str().unwrap_or(""))));

            assert_selected_text(manifest, &record, collection, source_number)?;
            records.push(Value::Object(record));
        }
    }
    Ok(records)
}

/// Rebuilds `data/hadith-fallback.json` from the pinned upstream editions.
/// `fetch(edition, url)` returns the raw edition bytes. Returns the serialized file text.
pub fn build_hadith_fallback(manifest: &Value, fetch: impl Fn(&str, &str) -> Result<Vec<u8>, String>) -> Result<(String, usize), String> {
    let source = &manifest["source"];
    let repository = source["repository"].as_str().unwrap_or("");
    let commit = source["commit"].as_str().unwrap_or("");
    let source_url = |path: &str| format!("{repository}/raw/{commit}/{path}");

    let editions_manifest = source["editions"].as_object().cloned().unwrap_or_default();
    let mut editions = HashMap::new();
    let mut source_editions = Map::new();
    for (edition, details) in &editions_manifest {
        let url = source_url(details["path"].as_str().unwrap_or(""));
        let bytes = fetch(edition, &url)?;
        let digest: String = Sha256::digest(&bytes).iter().map(|b| format!("{b:02x}")).collect();
        let expected = details["sha256"].as_str().unwrap_or("");
        if digest != expected {
            return Err(format!("SHA-256 mismatch for {edition}: expected {expected}, found {digest}"));
        }
        let parsed: Value = serde_json::from_slice(&bytes).map_err(|e| format!("Invalid JSON in {edition}: {e}"))?;
        editions.insert(edition.clone(), parsed);

        let mut entry = Map::new();
        entry.insert("url".into(), json!(url));
        entry.insert("sha256".into(), details["sha256"].clone());
        entry.insert("language".into(), details["language"].clone());
        entry.insert("translator".into(), details["translator"].clone());
        source_editions.insert(edition.clone(), Value::Object(entry));
    }

    let mut provenance = Map::new();
    provenance.insert("source".into(), source["name"].clone());
    provenance.insert("repository".into(), json!(repository));
    provenance.insert("commit".into(), json!(commit));
    provenance.insert("commitUrl".into(), json!(format!("{repository}/commit/{commit}")));
    provenance.insert("license".into(), source["license"].clone());
    provenance.insert("licenseUrl".into(), json!(format!("{repository}/blob/{commit}/LICENSE")));
    provenance.insert("selectionManifest".into(), json!(SELECTION_MANIFEST));
    provenance.insert("sourceEditions".into(), Value::Object(source_editions));

    let hadiths = build_records(manifest, &editions)?;
    let count = hadiths.len();
    let mut output = Map::new();
    output.insert("schemaVersion".into(), json!(1));
    output.insert("provenance".into(), Value::Object(provenance));
    output.insert("selectionPolicy".into(), manifest["policy"].clone());
    output.insert("hadiths".into(), Value::Array(hadiths));
    let serialized = serde_json::to_string_pretty(&Value::Object(output)).map_err(|e| e.to_string())?;
    Ok((serialized + "\n", count))
}
