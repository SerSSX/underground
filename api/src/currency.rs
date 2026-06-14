//! Currency reference (TASK-017 M5). Maps gazetteer countries to ISO 4217 codes,
//! validates currency codes, and exposes display symbols — no external service.

/// Currency for an unknown / missing country. UAH (the launch market) rather than a
/// neutral default, so events whose city doesn't resolve don't regress to a foreign
/// currency — international cities that DO resolve still get their own currency.
pub const DEFAULT: &str = "UAH";

/// ISO 4217 code for a gazetteer country (see migration 0015), falling back to USD.
/// Country strings must match the gazetteer's `country` column exactly.
pub fn for_country(country: Option<&str>) -> &'static str {
    match country {
        Some("Україна") => "UAH",
        Some("Poland") => "PLN",
        // Eurozone — one arm, many countries.
        Some(
            "Germany" | "France" | "Netherlands" | "Belgium" | "Spain" | "Portugal" | "Italy" | "Austria" | "Finland"
            | "Ireland" | "Greece" | "Lithuania" | "Latvia" | "Estonia",
        ) => "EUR",
        Some("United Kingdom") => "GBP",
        Some("Czechia") => "CZK",
        Some("Hungary") => "HUF",
        Some("Switzerland") => "CHF",
        Some("Sweden") => "SEK",
        Some("Denmark") => "DKK",
        Some("Norway") => "NOK",
        Some("Turkey") => "TRY",
        Some("Romania") => "RON",
        Some("Bulgaria") => "BGN",
        Some("USA") => "USD",
        Some("Canada") => "CAD",
        Some("Mexico") => "MXN",
        Some("Brazil") => "BRL",
        Some("Argentina") => "ARS",
        Some("Japan") => "JPY",
        Some("South Korea") => "KRW",
        Some("China") => "CNY",
        Some("Singapore") => "SGD",
        Some("Thailand") => "THB",
        Some("India") => "INR",
        Some("UAE") => "AED",
        Some("Israel") => "ILS",
        Some("Egypt") => "EGP",
        Some("South Africa") => "ZAR",
        Some("Nigeria") => "NGN",
        Some("Australia") => "AUD",
        Some("New Zealand") => "NZD",
        _ => DEFAULT,
    }
}

/// Display symbol for a currency code, if the platform supports it.
pub fn symbol(code: &str) -> Option<&'static str> {
    let s = match code {
        "UAH" => "₴",
        "USD" | "MXN" | "ARS" => "$",
        "EUR" => "€",
        "GBP" => "£",
        "PLN" => "zł",
        "CZK" => "Kč",
        "HUF" => "Ft",
        "CHF" => "CHF",
        "SEK" | "DKK" | "NOK" => "kr",
        "TRY" => "₺",
        "RON" => "lei",
        "BGN" => "лв",
        "CAD" => "C$",
        "BRL" => "R$",
        "JPY" | "CNY" => "¥",
        "KRW" => "₩",
        "SGD" => "S$",
        "THB" => "฿",
        "INR" => "₹",
        "AED" => "AED",
        "ILS" => "₪",
        "EGP" => "E£",
        "ZAR" => "R",
        "NGN" => "₦",
        "AUD" => "A$",
        "NZD" => "NZ$",
        _ => return None,
    };
    Some(s)
}

/// Whether a currency code is one the platform accepts.
pub fn is_supported(code: &str) -> bool {
    symbol(code).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_gazetteer_country_maps_to_its_currency() {
        // One representative per distinct currency arm (covers every match arm).
        let cases = [
            ("Україна", "UAH"),
            ("Poland", "PLN"),
            ("Germany", "EUR"),
            ("France", "EUR"),
            ("Netherlands", "EUR"),
            ("Belgium", "EUR"),
            ("Spain", "EUR"),
            ("Portugal", "EUR"),
            ("Italy", "EUR"),
            ("Austria", "EUR"),
            ("Finland", "EUR"),
            ("Ireland", "EUR"),
            ("Greece", "EUR"),
            ("Lithuania", "EUR"),
            ("Latvia", "EUR"),
            ("Estonia", "EUR"),
            ("United Kingdom", "GBP"),
            ("Czechia", "CZK"),
            ("Hungary", "HUF"),
            ("Switzerland", "CHF"),
            ("Sweden", "SEK"),
            ("Denmark", "DKK"),
            ("Norway", "NOK"),
            ("Turkey", "TRY"),
            ("Romania", "RON"),
            ("Bulgaria", "BGN"),
            ("USA", "USD"),
            ("Canada", "CAD"),
            ("Mexico", "MXN"),
            ("Brazil", "BRL"),
            ("Argentina", "ARS"),
            ("Japan", "JPY"),
            ("South Korea", "KRW"),
            ("China", "CNY"),
            ("Singapore", "SGD"),
            ("Thailand", "THB"),
            ("India", "INR"),
            ("UAE", "AED"),
            ("Israel", "ILS"),
            ("Egypt", "EGP"),
            ("South Africa", "ZAR"),
            ("Nigeria", "NGN"),
            ("Australia", "AUD"),
            ("New Zealand", "NZD"),
        ];
        for (country, code) in cases {
            assert_eq!(for_country(Some(country)), code, "country {country}");
            assert!(symbol(code).is_some(), "symbol for {code}");
            assert!(is_supported(code), "supported {code}");
        }
    }

    #[test]
    fn unknown_country_and_code_fall_back() {
        assert_eq!(for_country(None), DEFAULT);
        assert_eq!(for_country(Some("Atlantis")), DEFAULT);
        assert!(symbol("XYZ").is_none());
        assert!(!is_supported("XYZ"));
    }
}
