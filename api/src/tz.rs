//! Coordinates → IANA time zone (TASK-303 G1).
//!
//! The answer comes from `tzf-rs`: time-zone boundary polygons from OpenStreetMap (ODbL-1.0,
//! accepted by the owner on 2026-11-13). Where the polygons give no land zone (open sea, or an
//! `Etc/*` ocean zone), the place's ISO country code decides: one zone for a single-zone
//! country, the nearest reference city for a country that spans several zones. Zone names are
//! IANA tzdb identifiers (public domain); the fallback tables below are our own. An explicit
//! zone sent by the organiser always wins over either.

use std::sync::OnceLock;

fn finder() -> &'static tzf_rs::DefaultFinder {
    static FINDER: OnceLock<tzf_rs::DefaultFinder> = OnceLock::new();
    FINDER.get_or_init(tzf_rs::DefaultFinder::new)
}

/// Load the boundary data now (it takes a moment) rather than on the first request.
pub fn warm() {
    let _ = finder();
}

/// The land zone at a point from the boundary polygons; `None` at sea, for an `Etc/*` ocean
/// zone, or for a name the bundled tz database cannot parse.
pub fn zone_at_point(lat: f64, lng: f64) -> Option<&'static str> {
    if !(lat.is_finite() && lng.is_finite()) {
        return None;
    }
    let zone = finder().get_tz_name(lng, lat);
    let usable = !zone.is_empty() && !zone.starts_with("Etc/") && zone.parse::<chrono_tz::Tz>().is_ok();
    usable.then_some(zone)
}

/// The zone at a point: the polygons first, then the country fallback.
pub fn zone_for_point(country_code: Option<&str>, lat: f64, lng: f64) -> Option<&'static str> {
    zone_at_point(lat, lng).or_else(|| country_code.and_then(|cc| zone_for(cc, lat, lng)))
}

/// Countries with one zone (or several zones that always share one offset).
#[rustfmt::skip]
const SINGLE: &[(&str, &str)] = &[
    ("AD", "Europe/Andorra"), ("AE", "Asia/Dubai"), ("AF", "Asia/Kabul"), ("AG", "America/Antigua"),
    ("AI", "America/Anguilla"), ("AL", "Europe/Tirane"), ("AM", "Asia/Yerevan"), ("AO", "Africa/Luanda"),
    ("AR", "America/Argentina/Buenos_Aires"), ("AS", "Pacific/Pago_Pago"), ("AT", "Europe/Vienna"),
    ("AW", "America/Aruba"), ("AX", "Europe/Mariehamn"), ("AZ", "Asia/Baku"), ("BA", "Europe/Sarajevo"),
    ("BB", "America/Barbados"), ("BD", "Asia/Dhaka"), ("BE", "Europe/Brussels"), ("BF", "Africa/Ouagadougou"),
    ("BG", "Europe/Sofia"), ("BH", "Asia/Bahrain"), ("BI", "Africa/Bujumbura"), ("BJ", "Africa/Porto-Novo"),
    ("BL", "America/St_Barthelemy"), ("BM", "Atlantic/Bermuda"), ("BN", "Asia/Brunei"), ("BO", "America/La_Paz"),
    ("BQ", "America/Kralendijk"), ("BS", "America/Nassau"), ("BT", "Asia/Thimphu"), ("BW", "Africa/Gaborone"),
    ("BY", "Europe/Minsk"), ("BZ", "America/Belize"), ("CC", "Indian/Cocos"), ("CF", "Africa/Bangui"),
    ("CG", "Africa/Brazzaville"), ("CH", "Europe/Zurich"), ("CI", "Africa/Abidjan"), ("CK", "Pacific/Rarotonga"),
    ("CM", "Africa/Douala"), ("CN", "Asia/Shanghai"), ("CO", "America/Bogota"), ("CR", "America/Costa_Rica"),
    ("CU", "America/Havana"), ("CV", "Atlantic/Cape_Verde"), ("CW", "America/Curacao"),
    ("CX", "Indian/Christmas"), ("CY", "Asia/Nicosia"), ("CZ", "Europe/Prague"), ("DE", "Europe/Berlin"),
    ("DJ", "Africa/Djibouti"), ("DK", "Europe/Copenhagen"), ("DM", "America/Dominica"),
    ("DO", "America/Santo_Domingo"), ("DZ", "Africa/Algiers"), ("EE", "Europe/Tallinn"), ("EG", "Africa/Cairo"),
    ("EH", "Africa/El_Aaiun"), ("ER", "Africa/Asmara"), ("ET", "Africa/Addis_Ababa"), ("FI", "Europe/Helsinki"),
    ("FJ", "Pacific/Fiji"), ("FK", "Atlantic/Stanley"), ("FO", "Atlantic/Faroe"), ("GA", "Africa/Libreville"),
    ("GB", "Europe/London"), ("GD", "America/Grenada"), ("GE", "Asia/Tbilisi"), ("GF", "America/Cayenne"),
    ("GG", "Europe/Guernsey"), ("GH", "Africa/Accra"), ("GI", "Europe/Gibraltar"), ("GM", "Africa/Banjul"),
    ("GN", "Africa/Conakry"), ("GP", "America/Guadeloupe"), ("GQ", "Africa/Malabo"), ("GR", "Europe/Athens"),
    ("GT", "America/Guatemala"), ("GU", "Pacific/Guam"), ("GW", "Africa/Bissau"), ("GY", "America/Guyana"),
    ("HK", "Asia/Hong_Kong"), ("HN", "America/Tegucigalpa"), ("HR", "Europe/Zagreb"),
    ("HT", "America/Port-au-Prince"), ("HU", "Europe/Budapest"), ("IE", "Europe/Dublin"),
    ("IL", "Asia/Jerusalem"), ("IM", "Europe/Isle_of_Man"), ("IN", "Asia/Kolkata"), ("IQ", "Asia/Baghdad"),
    ("IR", "Asia/Tehran"), ("IS", "Atlantic/Reykjavik"), ("IT", "Europe/Rome"), ("JE", "Europe/Jersey"),
    ("JM", "America/Jamaica"), ("JO", "Asia/Amman"), ("JP", "Asia/Tokyo"), ("KE", "Africa/Nairobi"),
    ("KG", "Asia/Bishkek"), ("KH", "Asia/Phnom_Penh"), ("KM", "Indian/Comoro"), ("KN", "America/St_Kitts"),
    ("KP", "Asia/Pyongyang"), ("KR", "Asia/Seoul"), ("KW", "Asia/Kuwait"), ("KY", "America/Cayman"),
    ("LA", "Asia/Vientiane"), ("LB", "Asia/Beirut"), ("LC", "America/St_Lucia"), ("LI", "Europe/Vaduz"),
    ("LK", "Asia/Colombo"), ("LR", "Africa/Monrovia"), ("LS", "Africa/Maseru"), ("LT", "Europe/Vilnius"),
    ("LU", "Europe/Luxembourg"), ("LV", "Europe/Riga"), ("LY", "Africa/Tripoli"), ("MA", "Africa/Casablanca"),
    ("MC", "Europe/Monaco"), ("MD", "Europe/Chisinau"), ("ME", "Europe/Podgorica"), ("MF", "America/Marigot"),
    ("MG", "Indian/Antananarivo"), ("MH", "Pacific/Majuro"), ("MK", "Europe/Skopje"), ("ML", "Africa/Bamako"),
    ("MM", "Asia/Yangon"), ("MO", "Asia/Macau"), ("MP", "Pacific/Saipan"), ("MQ", "America/Martinique"),
    ("MR", "Africa/Nouakchott"), ("MS", "America/Montserrat"), ("MT", "Europe/Malta"),
    ("MU", "Indian/Mauritius"), ("MV", "Indian/Maldives"), ("MW", "Africa/Blantyre"),
    ("MY", "Asia/Kuala_Lumpur"), ("MZ", "Africa/Maputo"), ("NA", "Africa/Windhoek"), ("NC", "Pacific/Noumea"),
    ("NE", "Africa/Niamey"), ("NF", "Pacific/Norfolk"), ("NG", "Africa/Lagos"), ("NI", "America/Managua"),
    ("NO", "Europe/Oslo"), ("NP", "Asia/Kathmandu"), ("NR", "Pacific/Nauru"), ("NU", "Pacific/Niue"),
    ("OM", "Asia/Muscat"), ("PA", "America/Panama"), ("PE", "America/Lima"), ("PH", "Asia/Manila"),
    ("PK", "Asia/Karachi"), ("PL", "Europe/Warsaw"), ("PM", "America/Miquelon"), ("PN", "Pacific/Pitcairn"),
    ("PR", "America/Puerto_Rico"), ("PS", "Asia/Gaza"), ("PW", "Pacific/Palau"), ("PY", "America/Asuncion"),
    ("QA", "Asia/Qatar"), ("RE", "Indian/Reunion"), ("RO", "Europe/Bucharest"), ("RS", "Europe/Belgrade"),
    ("RW", "Africa/Kigali"), ("SA", "Asia/Riyadh"), ("SB", "Pacific/Guadalcanal"), ("SC", "Indian/Mahe"),
    ("SD", "Africa/Khartoum"), ("SE", "Europe/Stockholm"), ("SG", "Asia/Singapore"),
    ("SH", "Atlantic/St_Helena"), ("SI", "Europe/Ljubljana"), ("SJ", "Arctic/Longyearbyen"),
    ("SK", "Europe/Bratislava"), ("SL", "Africa/Freetown"), ("SM", "Europe/San_Marino"), ("SN", "Africa/Dakar"),
    ("SO", "Africa/Mogadishu"), ("SR", "America/Paramaribo"), ("SS", "Africa/Juba"), ("ST", "Africa/Sao_Tome"),
    ("SV", "America/El_Salvador"), ("SX", "America/Lower_Princes"), ("SY", "Asia/Damascus"),
    ("SZ", "Africa/Mbabane"), ("TC", "America/Grand_Turk"), ("TD", "Africa/Ndjamena"),
    ("TF", "Indian/Kerguelen"), ("TG", "Africa/Lome"), ("TH", "Asia/Bangkok"), ("TJ", "Asia/Dushanbe"),
    ("TK", "Pacific/Fakaofo"), ("TL", "Asia/Dili"), ("TM", "Asia/Ashgabat"), ("TN", "Africa/Tunis"),
    ("TO", "Pacific/Tongatapu"), ("TR", "Europe/Istanbul"), ("TT", "America/Port_of_Spain"),
    ("TV", "Pacific/Funafuti"), ("TW", "Asia/Taipei"), ("TZ", "Africa/Dar_es_Salaam"), ("UA", "Europe/Kyiv"),
    ("UG", "Africa/Kampala"), ("UY", "America/Montevideo"), ("UZ", "Asia/Tashkent"), ("VA", "Europe/Vatican"),
    ("VC", "America/St_Vincent"), ("VE", "America/Caracas"), ("VG", "America/Tortola"),
    ("VI", "America/St_Thomas"), ("VN", "Asia/Ho_Chi_Minh"), ("VU", "Pacific/Efate"), ("WF", "Pacific/Wallis"),
    ("WS", "Pacific/Apia"), ("XK", "Europe/Belgrade"), ("YE", "Asia/Aden"), ("YT", "Indian/Mayotte"),
    ("ZA", "Africa/Johannesburg"), ("ZM", "Africa/Lusaka"), ("ZW", "Africa/Harare"),
];

/// Countries with several offsets: reference cities, the nearest one wins.
#[rustfmt::skip]
const MULTI: &[(&str, &str, f64, f64)] = &[
    ("US", "America/New_York", 40.71, -74.01), ("US", "America/New_York", 42.36, -71.06),
    ("US", "America/New_York", 33.75, -84.39), ("US", "America/New_York", 25.76, -80.19),
    ("US", "America/New_York", 27.95, -82.46), ("US", "America/New_York", 40.44, -79.99),
    ("US", "America/New_York", 38.91, -77.04), ("US", "America/New_York", 35.23, -80.84),
    ("US", "America/Detroit", 42.33, -83.05), ("US", "America/Indiana/Indianapolis", 39.77, -86.16),
    ("US", "America/Kentucky/Louisville", 38.25, -85.76), ("US", "America/Chicago", 41.88, -87.63),
    ("US", "America/Chicago", 36.16, -86.78), ("US", "America/Chicago", 32.78, -96.80),
    ("US", "America/Chicago", 29.76, -95.37), ("US", "America/Chicago", 30.27, -97.74),
    ("US", "America/Chicago", 44.98, -93.27), ("US", "America/Chicago", 39.10, -94.58),
    ("US", "America/Chicago", 29.95, -90.07), ("US", "America/Chicago", 30.42, -87.22),
    ("US", "America/Chicago", 38.63, -90.20), ("US", "America/Denver", 39.74, -104.99),
    ("US", "America/Denver", 40.76, -111.89), ("US", "America/Denver", 35.08, -106.65),
    ("US", "America/Denver", 31.76, -106.49), ("US", "America/Boise", 43.62, -116.20),
    ("US", "America/Phoenix", 33.45, -112.07), ("US", "America/Phoenix", 32.22, -110.97),
    ("US", "America/Los_Angeles", 34.05, -118.24), ("US", "America/Los_Angeles", 37.77, -122.42),
    ("US", "America/Los_Angeles", 47.61, -122.33), ("US", "America/Los_Angeles", 45.52, -122.68),
    ("US", "America/Los_Angeles", 36.17, -115.14), ("US", "America/Los_Angeles", 32.72, -117.16),
    ("US", "America/Anchorage", 61.22, -149.90), ("US", "America/Juneau", 58.30, -134.42),
    ("US", "Pacific/Honolulu", 21.31, -157.86),
    ("CA", "America/Toronto", 43.65, -79.38), ("CA", "America/Toronto", 45.50, -73.57),
    ("CA", "America/Toronto", 45.42, -75.70), ("CA", "America/Toronto", 46.81, -71.21),
    ("CA", "America/Toronto", 48.38, -89.25), ("CA", "America/Halifax", 44.65, -63.57),
    ("CA", "America/Moncton", 46.09, -64.78), ("CA", "America/St_Johns", 47.56, -52.71),
    ("CA", "America/Winnipeg", 49.90, -97.14), ("CA", "America/Regina", 50.45, -104.62),
    ("CA", "America/Regina", 52.13, -106.67), ("CA", "America/Edmonton", 53.55, -113.49),
    ("CA", "America/Edmonton", 51.05, -114.07), ("CA", "America/Edmonton", 62.45, -114.37),
    ("CA", "America/Vancouver", 49.28, -123.12), ("CA", "America/Vancouver", 48.43, -123.37),
    ("CA", "America/Whitehorse", 60.72, -135.06), ("CA", "America/Iqaluit", 63.75, -68.52),
    ("MX", "America/Mexico_City", 19.43, -99.13), ("MX", "America/Mexico_City", 20.67, -103.35),
    ("MX", "America/Monterrey", 25.69, -100.32), ("MX", "America/Cancun", 21.16, -86.85),
    ("MX", "America/Chihuahua", 28.63, -106.07), ("MX", "America/Ciudad_Juarez", 31.69, -106.42),
    ("MX", "America/Hermosillo", 29.07, -110.96), ("MX", "America/Mazatlan", 23.25, -106.41),
    ("MX", "America/Mazatlan", 24.14, -110.31), ("MX", "America/Tijuana", 32.51, -117.04),
    ("BR", "America/Sao_Paulo", -23.55, -46.63), ("BR", "America/Sao_Paulo", -22.91, -43.17),
    ("BR", "America/Sao_Paulo", -15.79, -47.88), ("BR", "America/Sao_Paulo", -30.03, -51.23),
    ("BR", "America/Sao_Paulo", -19.92, -43.94), ("BR", "America/Sao_Paulo", -25.43, -49.27),
    ("BR", "America/Bahia", -12.97, -38.50), ("BR", "America/Recife", -8.05, -34.88),
    ("BR", "America/Fortaleza", -3.73, -38.53), ("BR", "America/Belem", -1.46, -48.50),
    ("BR", "America/Manaus", -3.12, -60.02), ("BR", "America/Cuiaba", -15.60, -56.10),
    ("BR", "America/Campo_Grande", -20.47, -54.62), ("BR", "America/Porto_Velho", -8.76, -63.90),
    ("BR", "America/Rio_Branco", -9.97, -67.81), ("BR", "America/Boa_Vista", 2.82, -60.67),
    ("BR", "America/Noronha", -3.85, -32.42),
    ("RU", "Europe/Kaliningrad", 54.71, 20.51), ("RU", "Europe/Moscow", 55.76, 37.62),
    ("RU", "Europe/Moscow", 59.94, 30.31), ("RU", "Europe/Moscow", 55.79, 49.12),
    ("RU", "Europe/Moscow", 43.60, 39.73), ("RU", "Europe/Volgograd", 48.71, 44.51),
    ("RU", "Europe/Samara", 53.20, 50.15), ("RU", "Asia/Yekaterinburg", 56.84, 60.60),
    ("RU", "Asia/Omsk", 54.99, 73.37), ("RU", "Asia/Novosibirsk", 55.03, 82.92),
    ("RU", "Asia/Krasnoyarsk", 56.01, 92.87), ("RU", "Asia/Irkutsk", 52.29, 104.28),
    ("RU", "Asia/Yakutsk", 62.03, 129.73), ("RU", "Asia/Vladivostok", 43.12, 131.89),
    ("RU", "Asia/Magadan", 59.56, 150.80), ("RU", "Asia/Kamchatka", 53.04, 158.65),
    ("AU", "Australia/Sydney", -33.87, 151.21), ("AU", "Australia/Sydney", -35.28, 149.13),
    ("AU", "Australia/Melbourne", -37.81, 144.96), ("AU", "Australia/Brisbane", -27.47, 153.03),
    ("AU", "Australia/Brisbane", -16.92, 145.77), ("AU", "Australia/Adelaide", -34.93, 138.60),
    ("AU", "Australia/Perth", -31.95, 115.86), ("AU", "Australia/Darwin", -12.46, 130.84),
    ("AU", "Australia/Hobart", -42.88, 147.33),
    ("ID", "Asia/Jakarta", -6.21, 106.85), ("ID", "Asia/Jakarta", -7.25, 112.75),
    ("ID", "Asia/Pontianak", -0.03, 109.33), ("ID", "Asia/Makassar", -5.15, 119.43),
    ("ID", "Asia/Makassar", -8.65, 115.22), ("ID", "Asia/Jayapura", -2.53, 140.72),
    ("KZ", "Asia/Almaty", 43.24, 76.95), ("KZ", "Asia/Almaty", 51.17, 71.43),
    ("KZ", "Asia/Aqtobe", 50.28, 57.17), ("KZ", "Asia/Aqtau", 43.65, 51.17), ("KZ", "Asia/Oral", 51.23, 51.37),
    ("MN", "Asia/Ulaanbaatar", 47.89, 106.91), ("MN", "Asia/Hovd", 48.00, 91.64),
    ("CD", "Africa/Kinshasa", -4.44, 15.27), ("CD", "Africa/Lubumbashi", -11.66, 27.48),
    ("CL", "America/Santiago", -33.45, -70.67), ("CL", "America/Punta_Arenas", -53.16, -70.92),
    ("CL", "Pacific/Easter", -27.11, -109.35),
    ("EC", "America/Guayaquil", -2.17, -79.92), ("EC", "America/Guayaquil", -0.18, -78.47),
    ("EC", "Pacific/Galapagos", -0.74, -90.31),
    ("ES", "Europe/Madrid", 40.42, -3.70), ("ES", "Europe/Madrid", 41.39, 2.17),
    ("ES", "Europe/Madrid", 37.39, -5.98), ("ES", "Atlantic/Canary", 28.12, -15.43),
    ("ES", "Atlantic/Canary", 28.47, -16.25), ("ES", "Africa/Ceuta", 35.89, -5.32),
    ("PT", "Europe/Lisbon", 38.72, -9.14), ("PT", "Europe/Lisbon", 41.15, -8.61),
    ("PT", "Atlantic/Madeira", 32.65, -16.91), ("PT", "Atlantic/Azores", 37.74, -25.67),
    ("FR", "Europe/Paris", 48.86, 2.35), ("FR", "Europe/Paris", 43.30, 5.37), ("FR", "Europe/Paris", 44.84, -0.58),
    ("FR", "Indian/Reunion", -20.88, 55.45), ("FR", "America/Guadeloupe", 16.24, -61.53),
    ("FR", "America/Martinique", 14.60, -61.07), ("FR", "America/Cayenne", 4.92, -52.31),
    ("FR", "Indian/Mayotte", -12.78, 45.23), ("FR", "Pacific/Tahiti", -17.53, -149.57),
    ("FR", "Pacific/Noumea", -22.27, 166.44),
    ("NL", "Europe/Amsterdam", 52.37, 4.90), ("NL", "America/Kralendijk", 12.15, -68.27),
    ("NZ", "Pacific/Auckland", -36.85, 174.76), ("NZ", "Pacific/Auckland", -43.53, 172.64),
    ("NZ", "Pacific/Chatham", -43.95, -176.56),
    ("GL", "America/Nuuk", 64.18, -51.69), ("GL", "America/Scoresbysund", 70.49, -21.97),
    ("GL", "America/Thule", 76.53, -68.70),
    ("KI", "Pacific/Tarawa", 1.33, 172.98), ("KI", "Pacific/Kanton", -2.80, -171.70),
    ("KI", "Pacific/Kiritimati", 1.87, -157.40),
    ("FM", "Pacific/Chuuk", 7.45, 151.85), ("FM", "Pacific/Pohnpei", 6.96, 158.21),
    ("FM", "Pacific/Kosrae", 5.33, 163.00),
    ("PF", "Pacific/Tahiti", -17.53, -149.57), ("PF", "Pacific/Marquesas", -9.00, -139.50),
    ("PF", "Pacific/Gambier", -23.10, -134.97),
    ("PG", "Pacific/Port_Moresby", -9.44, 147.18), ("PG", "Pacific/Bougainville", -6.23, 155.57),
];

/// Great-circle distance in km.
pub fn haversine_km(lat1: f64, lng1: f64, lat2: f64, lng2: f64) -> f64 {
    let (p1, p2) = (lat1.to_radians(), lat2.to_radians());
    let dp = (lat2 - lat1).to_radians();
    let dl = (lng2 - lng1).to_radians();
    let a = (dp / 2.0).sin().powi(2) + p1.cos() * p2.cos() * (dl / 2.0).sin().powi(2);
    2.0 * 6371.0 * a.sqrt().min(1.0).asin()
}

/// Fallback only: IANA zone for a point in the country `country_code` (ISO 3166-1 alpha-2, any
/// case) from the tables above. `None` for a country the tables do not know.
pub fn zone_for(country_code: &str, lat: f64, lng: f64) -> Option<&'static str> {
    let cc = country_code.trim().to_ascii_uppercase();
    let nearest = MULTI
        .iter()
        .filter(|(c, ..)| *c == cc)
        .map(|(_, zone, zlat, zlng)| (haversine_km(lat, lng, *zlat, *zlng), *zone))
        .min_by(|a, b| a.0.total_cmp(&b.0));
    if let Some((_, zone)) = nearest {
        return Some(zone);
    }
    SINGLE.iter().find(|(c, _)| *c == cc).map(|(_, zone)| *zone)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_zone_is_a_valid_iana_name() {
        let zones = SINGLE.iter().map(|(_, z)| *z).chain(MULTI.iter().map(|(_, z, ..)| *z));
        for z in zones {
            assert!(z.parse::<chrono_tz::Tz>().is_ok(), "{z} is not in the tz database");
        }
    }

    #[test]
    fn country_codes_are_well_formed_and_unambiguous() {
        let well_formed = |c: &str| c.len() == 2 && c.bytes().all(|b| b.is_ascii_uppercase());
        for (i, (c, _)) in SINGLE.iter().enumerate() {
            assert!(well_formed(c), "{c}");
            assert!(SINGLE[..i].iter().all(|(o, _)| o != c), "{c} listed twice");
            assert!(MULTI.iter().all(|(o, ..)| o != c), "{c} is in both tables");
        }
        assert!(MULTI.iter().all(|(c, ..)| well_formed(c)));
    }

    #[test]
    fn known_places_get_their_zone() {
        let cases = [
            ("PT", 38.72, -9.14, "Europe/Lisbon"),
            ("JP", 35.68, 139.65, "Asia/Tokyo"),
            ("UA", 50.45, 30.52, "Europe/Kyiv"),
            ("ua", 48.62, 22.29, "Europe/Kyiv"),
            ("US", 39.77, -86.16, "America/Indiana/Indianapolis"),
            ("US", 33.45, -112.07, "America/Phoenix"),
            ("US", 37.33, -121.89, "America/Los_Angeles"),
            ("US", 40.73, -73.94, "America/New_York"),
            ("AU", -31.95, 115.86, "Australia/Perth"),
            ("ES", 28.10, -15.41, "Atlantic/Canary"),
            ("RU", 43.12, 131.89, "Asia/Vladivostok"),
            ("BR", -3.10, -60.00, "America/Manaus"),
            ("CA", 49.26, -123.10, "America/Vancouver"),
        ];
        for (cc, lat, lng, want) in cases {
            assert_eq!(zone_for(cc, lat, lng), Some(want), "{cc} {lat},{lng}");
        }
        assert_eq!(zone_for("AQ", -75.0, 0.0), None, "unknown country → no guess");
        assert_eq!(zone_for("", 50.0, 30.0), None);
    }

    #[test]
    fn polygons_place_border_cities_in_their_zone() {
        let cases = [
            (50.4501, 30.5234, "Europe/Kyiv"),
            (35.6762, 139.6503, "Asia/Tokyo"),
            (38.7223, -9.1393, "Europe/Lisbon"),
            (42.9634, -85.6681, "America/Detroit"),       // Grand Rapids
            (30.4383, -84.2807, "America/New_York"),      // Tallahassee
            (35.2220, -101.8313, "America/Chicago"),      // Amarillo
            (53.3017, -60.3261, "America/Goose_Bay"),     // Goose Bay
            (55.7596, -120.2353, "America/Dawson_Creek"), // Dawson Creek
            (52.0317, 113.5010, "Asia/Chita"),            // Chita
            (46.9591, 142.7380, "Asia/Sakhalin"),         // Yuzhno-Sakhalinsk
            (51.5331, 46.0342, "Europe/Saratov"),         // Saratov
        ];
        for (lat, lng, want) in cases {
            assert_eq!(zone_at_point(lat, lng), Some(want), "{lat},{lng}");
            assert_eq!(zone_for_point(None, lat, lng), Some(want));
        }
    }

    #[test]
    fn the_sea_falls_back_to_the_country() {
        assert_eq!(zone_at_point(-30.0, -140.0), None, "mid-Pacific: no land zone");
        assert_eq!(zone_for_point(None, -30.0, -140.0), None);
        assert_eq!(zone_for_point(Some("PF"), -30.0, -140.0), Some("Pacific/Gambier"));
        assert_eq!(zone_at_point(f64::NAN, 0.0), None);
    }

    #[test]
    fn haversine_is_sane() {
        assert!(haversine_km(50.45, 30.52, 50.45, 30.52) < 0.001);
        let kyiv_london = haversine_km(50.4501, 30.5234, 51.5074, -0.1278);
        assert!((kyiv_london - 2133.0).abs() < 20.0, "{kyiv_london}");
    }
}
