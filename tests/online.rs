use chrono::{Days, NaiveDate};
use dulook::model;
use dulook::online::{self, OnlineQuery, PlaceSuggestion};
use dulook::settings::Settings;
use serde_json::json;

const YEAR: i32 = 2028;

fn isha(index: usize) -> String {
    format!("07:{:02} pm", index % 60)
}

/// A `get_period` table for 2028 shaped like salatcalendar's: header row, `*` DST day
/// markers, `-----` gaps, colspan cells, and one row with an extra cell.
fn period_html(skip: Option<usize>) -> String {
    let first = NaiveDate::from_ymd_opt(YEAR, 1, 1).unwrap();
    let mut html = String::from(
        "<table><thead><tr class=\"blue-gradient\"><th>Day</th><th>Date</th><th>Hijri</th><th>Fajr</th>\
         <th>Sunrise</th><th>Dhuhr</th><th>Asr</th><th colspan=\"2\">Maghrib</th><th colspan=\"2\">Isha</th></tr></thead><tbody>",
    );
    for index in 0..366 {
        if skip == Some(index) {
            continue;
        }
        let day = first.checked_add_days(Days::new(index as u64)).unwrap();
        let name = if index == 10 { format!("*{}", day.format("%A")) } else { day.format("%A").to_string() };
        let isha = if index == 0 || index == 100 { "-----".to_owned() } else { isha(index) };
        let extra = if index == 50 { "<td>08:00 pm</td>" } else { "" };
        html += &format!(
            "<tr >\n<th>{name}</th><td class=\"no-wrap\">{}</td><td>01/01/1450 (Muh) </td><td>05:00 am</td>\
             <td>06:30 am</td><td>12:40 pm</td><td>03:10 pm</td><td colspan=\"2\">06:00 pm</td>\
             <td colspan=\"2\" >{isha}</td>{extra}\n</tr>",
            day.format("%d/%m/%Y")
        );
    }
    html + "</tbody></table>"
}

#[test]
fn period_csv_normalises_a_whole_year() {
    let csv = online::period_csv(&period_html(None), YEAR).unwrap();
    assert!(!csv.ends_with('\n'));
    let lines: Vec<&str> = csv.split("\r\n").collect();
    assert_eq!(lines.len(), 367);
    assert_eq!(lines[0], "Day,Date,Hijri,Fajr,Sunrise,Dhuhr,Asr,Maghrib,Isha");
    assert_eq!(lines[1], format!("Saturday,01/01/2028,01/01/1450,05:00 am,06:30 am,12:40 pm,03:10 pm,06:00 pm,{}", isha(1)));
    assert!(lines[11].starts_with("Tuesday,11/01/2028,"), "{}", lines[11]);
    assert!(lines[101].ends_with(&format!(",{}", isha(99))), "{}", lines[101]);
    assert_eq!(lines[51].split(',').count(), 9);
    assert_eq!(model::parse_calendar(&csv).unwrap().row_count(), 366);
}

#[test]
fn period_csv_rejects_a_missing_day() {
    assert!(online::period_csv(&period_html(Some(200)), YEAR).is_err());
    assert!(online::period_csv(&period_html(None), YEAR + 1).is_err());
}

fn place(country: &str, zip: &str, city: &str, street: &str, house_number: &str) -> OnlineQuery {
    OnlineQuery {
        country: country.into(),
        zip: zip.into(),
        city: city.into(),
        street: street.into(),
        house_number: house_number.into(),
        coordinates: None,
        pinned: false,
    }
}

#[test]
fn location_names_are_country_zip_city_street_house_number() {
    assert_eq!(online::slug(&place("de", "48527", "Nordhorn", "", "")), "DE_48527_Nordhorn");
    assert_eq!(online::slug(&place("EG", "", "سوهاج", "", "")), "EG_سوهاج");
    assert_eq!(online::slug(&place("DE", " ", " Bad  Bentheim ", "", "")), "DE_Bad-Bentheim");
    assert_eq!(
        online::slug(&place("DE", "48529", "Nordhorn", "Bentheimer Straße", "10a")),
        "DE_48529_Nordhorn_Bentheimer-Straße_10a"
    );
    assert_ne!(
        online::slug(&place("DE", "48527", "Nordhorn", "", "")),
        online::slug(&place("DE", "48529", "Nordhorn", "", ""))
    );
    assert_eq!(
        online::calendar_file(std::path::Path::new("/c"), "DE_48527_Nordhorn", 2027),
        std::path::Path::new("/c/DE_48527_Nordhorn/DE_48527_Nordhorn_2027.csv")
    );
}

#[test]
fn photon_addresses_match_the_typed_street_and_house_number() {
    let json = r#"{"features":[
        {"geometry":{"coordinates":[7.08,52.43]},"properties":{"type":"house","street":"Heseper Weg","housenumber":"10"}},
        {"geometry":{"coordinates":[7.0694,52.4318]},"properties":{"type":"street","name":"Bentheimer Straße"}},
        {"geometry":{"coordinates":[7.0691,52.4330]},"properties":{"type":"house","street":"Bentheimer Straße","housenumber":"10"}}
    ]}"#;
    assert_eq!(online::parse_address(json, "Bentheimer Str.", "10").unwrap(), Some((52.4330, 7.0691)));
    assert_eq!(online::parse_address(json, "bentheimer straße", "").unwrap(), Some((52.4318, 7.0694)));
    assert_eq!(online::parse_address(json, "Bentheimer Straße", "99").unwrap(), Some((52.4318, 7.0694)));
    assert_eq!(online::parse_address(json, "Nonexistentweg", "10").unwrap(), None);
}

#[test]
fn photon_postcode_results_carry_zip_and_city() {
    let json = r#"{"features":[{"geometry":{"coordinates":[7.04756,52.47219]},"properties":{"name":"48527","city":"Nordhorn","countrycode":"DE"}}]}"#;
    let suggestions = online::parse_photon(json, true).unwrap();
    assert_eq!(
        suggestions,
        [PlaceSuggestion {
            city: "Nordhorn".into(),
            zip: "48527".into(),
            street: String::new(),
            house_number: String::new(),
            country: "DE".into(),
            label: "48527 Nordhorn".into(),
            latitude: 52.47219,
            longitude: 7.04756,
        }]
    );
}

#[test]
fn photon_city_results_are_labelled_with_county_and_state_once() {
    let feature = r#"{"geometry":{"coordinates":[7.07,52.43]},"properties":{"name":"Nordhorn","county":"Landkreis Grafschaft Bentheim","state":"Lower Saxony","countrycode":"de"}}"#;
    let json = format!(r#"{{"features":[{feature},{feature}]}}"#);
    let suggestions = online::parse_photon(&json, false).unwrap();
    assert_eq!(suggestions.len(), 1);
    assert_eq!(suggestions[0].label, "Nordhorn, Landkreis Grafschaft Bentheim, Lower Saxony");
    assert_eq!((suggestions[0].city.as_str(), suggestions[0].zip.as_str(), suggestions[0].country.as_str()), ("Nordhorn", "", "DE"));
}

#[test]
fn street_suggestions_stay_in_the_typed_city() {
    let json = r#"{"features":[
        {"geometry":{"coordinates":[7.0694,52.4318]},"properties":{"type":"street","name":"Bentheimer Straße","postcode":"48529","city":"Nordhorn","countrycode":"DE"}},
        {"geometry":{"coordinates":[7.0979,52.3970]},"properties":{"type":"street","name":"Bentheimer Straße","postcode":"48529","city":"Nordhorn","countrycode":"DE"}},
        {"geometry":{"coordinates":[7.25,52.30]},"properties":{"type":"street","name":"Bentheimer Straße","postcode":"48465","city":"Schüttorf","countrycode":"DE"}},
        {"geometry":{"coordinates":[7.07,52.43]},"properties":{"type":"street","name":"Heseper Weg","postcode":"48529","city":"Nordhorn","countrycode":"DE"}}
    ]}"#;
    let streets = online::parse_streets(json, "Benthei", "nordhorn").unwrap();
    assert_eq!(streets.len(), 1);
    assert_eq!(streets[0].label, "Bentheimer Straße, 48529 Nordhorn");
    assert_eq!((streets[0].street.as_str(), streets[0].zip.as_str(), streets[0].city.as_str()), ("Bentheimer Straße", "48529", "Nordhorn"));
    assert_eq!(online::parse_streets(json, "Benthei", "").unwrap().len(), 2);
}

#[test]
fn house_suggestions_are_numbered_in_order_and_filtered_by_prefix() {
    let house = |street: &str, number: &str| {
        format!(
            r#"{{"geometry":{{"coordinates":[7.07,52.43]}},"properties":{{"type":"house","street":"{street}","housenumber":"{number}","postcode":"48529","city":"Nordhorn","countrycode":"DE"}}}}"#
        )
    };
    let response = |houses: &[String]| format!(r#"{{"features":[{}]}}"#, houses.join(","));
    let near = response(&[
        house("Bentheimer Straße", "12"),
        house("Bentheimer Straße", "2"),
        house("Heseper Weg", "1"),
        house("Bentheimer Straße", "10a"),
        house("Bentheimer Straße", "10"),
    ]);
    let exact = response(&[house("Bentheimer Straße", "1"), house("Bentheimer Straße", "10")]);
    let numbers = |prefix: &str| {
        online::parse_houses(&[exact.clone(), near.clone()], "Bentheimer Straße", prefix)
            .unwrap()
            .into_iter()
            .map(|suggestion| suggestion.house_number)
            .collect::<Vec<_>>()
    };
    assert_eq!(numbers(""), ["1", "2", "10", "10a", "12"]);
    assert_eq!(numbers("1"), ["1", "10", "10a", "12"]);
    assert_eq!(numbers("10A"), ["10a"]);
    let first = &online::parse_houses(&[near], "Bentheimer Str", "2").unwrap()[0];
    assert_eq!(first.label, "Bentheimer Straße 2, 48529 Nordhorn");
}

fn pinned(query: OnlineQuery, coordinates: Option<(f64, f64)>) -> OnlineQuery {
    OnlineQuery { coordinates, pinned: true, ..query }
}

#[test]
fn pinned_locations_are_named_by_their_coordinates_instead_of_the_street() {
    let alkenstiege = place("DE", "48529", "Nordhorn", "Alkenstiege", "10");
    assert_eq!(online::slug(&pinned(alkenstiege.clone(), Some((52.43301, 7.06913)))), "DE_48529_Nordhorn_52.433010_7.069130");
    assert_eq!(
        online::slug(&pinned(place("BR", "", "Rio de Janeiro", "", ""), Some((-22.9068, -43.1729)))),
        "BR_Rio-de-Janeiro_-22.906800_-43.172900"
    );
    assert_eq!(online::slug(&pinned(alkenstiege, None)), "DE_48529_Nordhorn_Alkenstiege_10");
}

#[test]
fn a_pin_is_filled_with_the_place_photon_finds_there() {
    let house = r#"{"features":[{"geometry":{"coordinates":[7.0691,52.4330]},"properties":{"type":"house",
        "name":"Hungeling","street":"Alkenstiege","housenumber":"10","postcode":"48529","city":"Nordhorn","countrycode":"DE"}}]}"#;
    let found = online::parse_place_at(house, (52.43301, 7.06913)).unwrap().unwrap();
    assert_eq!(
        (found.country.as_str(), found.city.as_str(), found.zip.as_str(), found.street.as_str(), found.house_number.as_str()),
        ("DE", "Nordhorn", "48529", "Alkenstiege", "10")
    );
    assert_eq!(found.label, "Alkenstiege 10, 48529 Nordhorn");
    assert_eq!((found.latitude, found.longitude), (52.43301, 7.06913));

    let locality = r#"{"features":[{"geometry":{"coordinates":[7.0,52.4]},"properties":{"type":"locality",
        "name":"Feld","housenumber":"3","city":"Nordhorn","countrycode":"de"}}]}"#;
    let found = online::parse_place_at(locality, (52.4, 7.0)).unwrap().unwrap();
    assert_eq!((found.country.as_str(), found.city.as_str(), found.street.as_str(), found.house_number.as_str()), ("DE", "Nordhorn", "", ""));

    assert_eq!(online::parse_place_at(r#"{"features":[]}"#, (54.3, 7.0)).unwrap(), None);
}

#[test]
fn pinned_coordinates_survive_a_street_while_suggested_ones_do_not() {
    let settings = |pin: bool| {
        let raw = json!({ "locationStreet": "Alkenstiege", "locationLatitude": "52.43301",
            "locationLongitude": "7.06913", "locationPinned": pin });
        Settings::from_raw(raw.as_object().unwrap(), std::path::Path::new("/p"), "/home/u")
    };
    let pinned = settings(true);
    assert_eq!((pinned.location_coordinates, pinned.location_pinned), (Some((52.43301, 7.06913)), true));
    let suggested = settings(false);
    assert_eq!((suggested.location_coordinates, suggested.location_pinned), (None, false));
}
