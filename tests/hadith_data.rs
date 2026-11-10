use std::collections::HashSet;
use std::sync::LazyLock;

use dulook::hadith_data::{HadithDataSummary, is_fragment, validate_hadith_data};
use dulook::jsutil::utf16_len;
use regex::Regex;
use serde_json::Value;
use unicode_normalization::UnicodeNormalization;

static DATA: LazyLock<Value> = LazyLock::new(|| serde_json::from_str(include_str!("../data/hadith-fallback.json")).unwrap());
static MANIFEST: LazyLock<Value> =
    LazyLock::new(|| serde_json::from_str(include_str!("../sources/hadith-api-selection.json")).unwrap());

fn records() -> &'static [Value] {
    DATA["hadiths"].as_array().unwrap()
}

fn text<'a>(record: &'a Value, key: &str) -> &'a str {
    record[key].as_str().unwrap()
}

fn policy(key: &str) -> usize {
    MANIFEST["policy"][key].as_u64().unwrap() as usize
}

#[test]
fn contains_exactly_100_logical_records_split_50_50() {
    assert_eq!(records().len(), 100);
    for collection in ["bukhari", "muslim"] {
        assert_eq!(records().iter().filter(|r| r["collection"] == collection).count(), 50);
    }
}

#[test]
fn every_record_has_a_nonempty_panel_sized_arabic_english_pair() {
    let arabic = Regex::new(r"[\x{0600}-\x{06FF}]").unwrap();
    let latin = Regex::new(r"[A-Za-z]").unwrap();
    for record in records() {
        let id = text(record, "id");
        let (ar, en) = (text(record, "arabic"), text(record, "english"));
        assert!(arabic.is_match(ar), "{id}");
        assert!(latin.is_match(en), "{id}");
        assert!(utf16_len(ar) >= policy("minimumArabicCharacters"), "{id}");
        assert!(utf16_len(ar) <= policy("maximumArabicCharacters"), "{id}");
        assert!(utf16_len(en) >= policy("minimumEnglishCharacters"), "{id}");
        assert!(utf16_len(en) <= policy("maximumEnglishCharacters"), "{id}");
        assert!(!is_fragment(en), "{id}");
    }
}

#[test]
fn ids_and_bilingual_texts_are_unique() {
    let normalize = |t: &str| t.nfkc().collect::<String>().split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase();
    let unique = |key: &str, normalized: bool| {
        records()
            .iter()
            .map(|r| if normalized { normalize(text(r, key)) } else { text(r, key).to_owned() })
            .collect::<HashSet<_>>()
            .len()
    };
    assert_eq!(unique("id", false), 100);
    assert_eq!(unique("english", true), 100);
    assert_eq!(unique("arabic", true), 100);
}

#[test]
fn canonical_and_in_book_references_are_useful_and_internally_consistent() {
    let number = Regex::new(r"^[0-9]+[a-z]?$").unwrap();
    for record in records() {
        let hadith_number = text(record, "hadithNumber");
        assert!(number.is_match(hadith_number), "{}", text(record, "id"));
        let reference = &record["reference"];
        assert_eq!(reference["canonical"], format!("{} {hadith_number}", text(record, "collectionName")));
        assert_eq!(reference["inBook"], format!("Book {}, Hadith {}", record["bookNumber"], record["inBookHadithNumber"]));
        assert_eq!(reference["url"], format!("https://sunnah.com/{}:{hadith_number}", text(record, "collection")));
        assert!(!text(record, "bookName").is_empty(), "{}", text(record, "id"));
    }
}

#[test]
fn provenance_is_immutable_and_matches_the_reviewed_source_selection() {
    let provenance = &DATA["provenance"];
    let commit = MANIFEST["source"]["commit"].as_str().unwrap();
    assert!(Regex::new("^[0-9a-f]{40}$").unwrap().is_match(provenance["commit"].as_str().unwrap()));
    assert_eq!(provenance["commit"], commit);
    assert_eq!(provenance["repository"], MANIFEST["source"]["repository"]);
    assert_eq!(provenance["selectionManifest"], "sources/hadith-api-selection.json");

    for (edition, details) in provenance["sourceEditions"].as_object().unwrap() {
        assert!(details["url"].as_str().unwrap().contains(&format!("/{commit}/")), "{edition}");
        assert_eq!(details["sha256"], MANIFEST["source"]["editions"][edition]["sha256"]);
    }

    for collection in ["bukhari", "muslim"] {
        let numbers: Vec<&Value> =
            records().iter().filter(|r| r["collection"] == collection).map(|r| &r["sourceHadithNumber"]).collect();
        let expected: Vec<&Value> = MANIFEST["selection"][collection].as_array().unwrap().iter().collect();
        assert_eq!(numbers, expected);
    }
}

#[test]
fn the_shared_runtime_validator_accepts_the_generated_artifact() {
    assert_eq!(
        validate_hadith_data(&DATA, &MANIFEST).unwrap(),
        HadithDataSummary { total: 100, bukhari: 50, muslim: 50 }
    );
}
