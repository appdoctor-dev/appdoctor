/// An App Store storefront: ISO 3166-1 alpha-2 code (lowercase, as the iTunes
/// APIs take it) and English name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Country {
    pub code: &'static str,
    pub name: &'static str,
}

/// Looks a storefront up by code, case-insensitively.
pub fn country(code: &str) -> Option<&'static Country> {
    COUNTRIES.iter().find(|country| country.code.eq_ignore_ascii_case(code))
}

macro_rules! countries {
    ($($code:literal => $name:literal),* $(,)?) => {
        /// App Store storefronts, sorted by name.
        pub static COUNTRIES: &[Country] = &[$(Country { code: $code, name: $name }),*];
    };
}

countries! {
    "af" => "Afghanistan", "al" => "Albania", "dz" => "Algeria", "ao" => "Angola",
    "ai" => "Anguilla", "ag" => "Antigua and Barbuda", "ar" => "Argentina", "am" => "Armenia",
    "au" => "Australia", "at" => "Austria", "az" => "Azerbaijan", "bs" => "Bahamas",
    "bh" => "Bahrain", "bb" => "Barbados", "by" => "Belarus", "be" => "Belgium",
    "bz" => "Belize", "bj" => "Benin", "bm" => "Bermuda", "bt" => "Bhutan",
    "bo" => "Bolivia", "ba" => "Bosnia and Herzegovina", "bw" => "Botswana", "br" => "Brazil",
    "vg" => "British Virgin Islands", "bn" => "Brunei", "bg" => "Bulgaria", "bf" => "Burkina Faso",
    "kh" => "Cambodia", "cm" => "Cameroon", "ca" => "Canada", "cv" => "Cape Verde",
    "ky" => "Cayman Islands", "td" => "Chad", "cl" => "Chile", "cn" => "China mainland",
    "co" => "Colombia", "cr" => "Costa Rica", "hr" => "Croatia", "cy" => "Cyprus",
    "cz" => "Czechia", "ci" => "Côte d'Ivoire", "cd" => "Democratic Republic of the Congo",
    "dk" => "Denmark", "dm" => "Dominica", "do" => "Dominican Republic", "ec" => "Ecuador",
    "eg" => "Egypt", "sv" => "El Salvador", "ee" => "Estonia", "sz" => "Eswatini",
    "fj" => "Fiji", "fi" => "Finland", "fr" => "France", "ga" => "Gabon",
    "gm" => "Gambia", "ge" => "Georgia", "de" => "Germany", "gh" => "Ghana",
    "gr" => "Greece", "gd" => "Grenada", "gt" => "Guatemala", "gw" => "Guinea-Bissau",
    "gy" => "Guyana", "hn" => "Honduras", "hk" => "Hong Kong", "hu" => "Hungary",
    "is" => "Iceland", "in" => "India", "id" => "Indonesia", "iq" => "Iraq",
    "ie" => "Ireland", "il" => "Israel", "it" => "Italy", "jm" => "Jamaica",
    "jp" => "Japan", "jo" => "Jordan", "kz" => "Kazakhstan", "ke" => "Kenya",
    "xk" => "Kosovo", "kw" => "Kuwait", "kg" => "Kyrgyzstan", "la" => "Laos",
    "lv" => "Latvia", "lb" => "Lebanon", "lr" => "Liberia", "ly" => "Libya",
    "lt" => "Lithuania", "lu" => "Luxembourg", "mo" => "Macao", "mg" => "Madagascar",
    "mw" => "Malawi", "my" => "Malaysia", "mv" => "Maldives", "ml" => "Mali",
    "mt" => "Malta", "mr" => "Mauritania", "mu" => "Mauritius", "mx" => "Mexico",
    "fm" => "Micronesia", "md" => "Moldova", "mn" => "Mongolia", "me" => "Montenegro",
    "ms" => "Montserrat", "ma" => "Morocco", "mz" => "Mozambique", "mm" => "Myanmar",
    "na" => "Namibia", "nr" => "Nauru", "np" => "Nepal", "nl" => "Netherlands",
    "nz" => "New Zealand", "ni" => "Nicaragua", "ne" => "Niger", "ng" => "Nigeria",
    "mk" => "North Macedonia", "no" => "Norway", "om" => "Oman", "pk" => "Pakistan",
    "pw" => "Palau", "pa" => "Panama", "pg" => "Papua New Guinea", "py" => "Paraguay",
    "pe" => "Peru", "ph" => "Philippines", "pl" => "Poland", "pt" => "Portugal",
    "qa" => "Qatar", "cg" => "Republic of the Congo", "ro" => "Romania", "ru" => "Russia",
    "rw" => "Rwanda", "kn" => "Saint Kitts and Nevis", "lc" => "Saint Lucia",
    "vc" => "Saint Vincent and the Grenadines", "st" => "São Tomé and Príncipe",
    "sa" => "Saudi Arabia", "sn" => "Senegal", "rs" => "Serbia", "sc" => "Seychelles",
    "sl" => "Sierra Leone", "sg" => "Singapore", "sk" => "Slovakia", "si" => "Slovenia",
    "sb" => "Solomon Islands", "za" => "South Africa", "kr" => "South Korea", "es" => "Spain",
    "lk" => "Sri Lanka", "sr" => "Suriname", "se" => "Sweden", "ch" => "Switzerland",
    "tw" => "Taiwan", "tj" => "Tajikistan", "tz" => "Tanzania", "th" => "Thailand",
    "to" => "Tonga", "tt" => "Trinidad and Tobago", "tn" => "Tunisia", "tr" => "Türkiye",
    "tm" => "Turkmenistan", "tc" => "Turks and Caicos Islands", "ug" => "Uganda",
    "ua" => "Ukraine", "ae" => "United Arab Emirates", "gb" => "United Kingdom",
    "us" => "United States", "uy" => "Uruguay", "uz" => "Uzbekistan", "vu" => "Vanuatu",
    "ve" => "Venezuela", "vn" => "Vietnam", "ye" => "Yemen", "zm" => "Zambia",
    "zw" => "Zimbabwe",
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_are_unique_lowercase_pairs() {
        let mut codes: Vec<_> = COUNTRIES.iter().map(|c| c.code).collect();
        assert!(
            codes
                .iter()
                .all(|c| c.len() == 2 && c.chars().all(|ch| ch.is_ascii_lowercase()))
        );
        codes.sort_unstable();
        codes.dedup();
        assert_eq!(codes.len(), COUNTRIES.len());
    }

    #[test]
    fn lookup_is_case_insensitive() {
        assert_eq!(country("US").unwrap().name, "United States");
        assert_eq!(country("gb").unwrap().name, "United Kingdom");
        assert!(country("zz").is_none());
    }
}
