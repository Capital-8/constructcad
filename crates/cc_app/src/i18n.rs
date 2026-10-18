//! Localization: a Fluent-compatible subset (`key = value` messages with
//! `{ $var }` placeholders). All languages are embedded at build time and the
//! active one can be switched at runtime.

use std::collections::HashMap;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    PtBr,
    En,
}

impl Language {
    pub const ALL: [Language; 2] = [Language::PtBr, Language::En];

    /// BCP 47 code, used for persistence.
    pub fn code(self) -> &'static str {
        match self {
            Language::PtBr => "pt-BR",
            Language::En => "en",
        }
    }

    /// Name in the language itself, so it is recognizable whatever is active.
    pub fn native_name(self) -> &'static str {
        match self {
            Language::PtBr => "Português (Brasil)",
            Language::En => "English",
        }
    }

    pub fn from_code(code: &str) -> Option<Language> {
        let code = code.to_ascii_lowercase();
        if code.starts_with("pt") {
            Some(Language::PtBr)
        } else if code.starts_with("en") {
            Some(Language::En)
        } else {
            None
        }
    }

    /// Language of the operating system, English when unsupported.
    pub fn from_system() -> Language {
        sys_locale::get_locale().and_then(|l| Language::from_code(&l)).unwrap_or(Language::En)
    }

    fn source(self) -> &'static str {
        match self {
            Language::PtBr => include_str!("../../../assets/i18n/pt-BR.ftl"),
            Language::En => include_str!("../../../assets/i18n/en.ftl"),
        }
    }

    fn index(self) -> usize {
        Language::ALL.iter().position(|l| *l == self).unwrap_or(0)
    }

    /// Whether numbers use a decimal comma.
    pub fn decimal_comma(self) -> bool {
        matches!(self, Language::PtBr)
    }
}

static CURRENT: AtomicUsize = AtomicUsize::new(0);

fn bundle(lang: Language) -> &'static HashMap<String, String> {
    static BUNDLES: OnceLock<Vec<HashMap<String, String>>> = OnceLock::new();
    &BUNDLES.get_or_init(|| Language::ALL.iter().map(|l| parse_messages(l.source())).collect())[lang.index()]
}

pub fn language() -> Language {
    Language::ALL[CURRENT.load(Ordering::Relaxed)]
}

/// Switch the active language (also sets the number format).
pub fn set_language(lang: Language) {
    CURRENT.store(lang.index(), Ordering::Relaxed);
    cc_views::format::set_decimal_comma(lang.decimal_comma());
}

/// Parse `key = value` lines; `#` starts a comment line.
pub fn parse_messages(source: &str) -> HashMap<String, String> {
    source
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter_map(|l| l.split_once('='))
        .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
        .collect()
}

/// Translate `key` if the active language defines it.
pub fn try_tr(key: &str) -> Option<String> {
    bundle(language()).get(key).cloned()
}

/// Translate `key`; falls back to Portuguese, then to the key itself so gaps are visible.
pub fn tr(key: &str) -> String {
    try_tr(key).or_else(|| bundle(Language::PtBr).get(key).cloned()).unwrap_or_else(|| key.to_string())
}

/// Translate `key`, substituting `{ $name }` placeholders.
pub fn tr_args(key: &str, args: &[(&str, String)]) -> String {
    let mut s = tr(key);
    for (name, value) in args {
        s = s.replace(&format!("{{ ${name} }}"), value);
    }
    s
}

/// `t!("key")` or `t!("key", "name" => value, ...)`.
#[macro_export]
macro_rules! t {
    ($key:literal) => {
        $crate::i18n::tr($key)
    };
    ($key:literal, $($name:literal => $value:expr),+ $(,)?) => {
        $crate::i18n::tr_args($key, &[$(($name, $value.to_string())),+])
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn placeholders() {
        let m = parse_messages("a = Olá { $name }!\n# comment\nb=x");
        assert_eq!(m["a"], "Olá { $name }!");
        assert_eq!(m["b"], "x");
    }

    #[test]
    fn language_codes() {
        assert_eq!(Language::from_code("pt_BR"), Some(Language::PtBr));
        assert_eq!(Language::from_code("en-US"), Some(Language::En));
        assert_eq!(Language::from_code("fr-FR"), None);
        for l in Language::ALL {
            assert_eq!(Language::from_code(l.code()), Some(l));
        }
    }

    fn placeholders_of(s: &str) -> BTreeSet<&str> {
        s.split("{ $").skip(1).filter_map(|p| p.split(" }").next()).collect()
    }

    /// Every language defines exactly the same keys with the same placeholders.
    #[test]
    fn languages_are_complete() {
        let reference = parse_messages(Language::PtBr.source());
        for lang in Language::ALL {
            let messages = parse_messages(lang.source());
            let missing: Vec<_> = reference.keys().filter(|k| !messages.contains_key(*k)).collect();
            let extra: Vec<_> = messages.keys().filter(|k| !reference.contains_key(*k)).collect();
            assert!(missing.is_empty() && extra.is_empty(), "{lang:?}: missing {missing:?}, extra {extra:?}");
            for (k, v) in &messages {
                assert_eq!(placeholders_of(v), placeholders_of(&reference[k]), "{lang:?}: placeholders of {k}");
            }
        }
    }

    /// Every `t!("...")` key used in the sources must exist in the translations.
    #[test]
    fn all_keys_translated() {
        let messages = parse_messages(Language::PtBr.source());
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut missing = Vec::new();
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.file_name().is_some_and(|n| n == "i18n.rs") {
                continue; // macro docs mention placeholder keys
            }
            let src = std::fs::read_to_string(path).unwrap();
            for (pos, _) in src.match_indices("t!(\"") {
                // Skip other macros ending in `t!`, such as `format!`.
                if src[..pos].chars().last().is_some_and(|c| c.is_alphanumeric() || c == '_') {
                    continue;
                }
                let key = src[pos + 4..].split('"').next().unwrap();
                if !messages.contains_key(key) {
                    missing.push(key.to_string());
                }
            }
        }
        // Keys built at runtime from the model crates.
        let runtime =
            cc_core::Category::ALL.map(|c| c.key()).into_iter().chain(cc_core::ViewCategory::ALL.map(|c| c.key()));
        missing.extend(runtime.filter(|k| !messages.contains_key(*k)).map(String::from));
        let project = cc_core::Project::from_template();
        let geom = cc_geom::ModelGeometry::build(&project);
        for c in cc_core::Category::ALL {
            for h in cc_views::schedule(&project, &geom, c, &Default::default()).headers {
                if !messages.contains_key(h) {
                    missing.push(h.to_string());
                }
            }
            let fields = cc_core::ScheduleField::available(c);
            missing.extend(fields.iter().map(|f| f.key()).filter(|k| !messages.contains_key(*k)).map(String::from));
        }
        let cube = crate::view_cube::KEYS;
        let materials = crate::material_browser::KEYS
            .into_iter()
            .chain(cc_core::MaterialClass::ALL.map(|c| c.key()))
            .chain(cc_core::Pattern::ALL.map(|p| p.key()))
            .chain(cc_core::material::Finish::ALL.map(|f| f.key()))
            .chain(cc_core::material::Behavior::ALL.map(|b| b.key()));
        for k in ["value-yes", "value-no"].into_iter().chain(cube).chain(materials) {
            if !messages.contains_key(k) {
                missing.push(k.to_string());
            }
        }
        missing.sort();
        missing.dedup();
        assert!(missing.is_empty(), "missing translations: {missing:?}");
    }

    /// The localized template resolves every name it asks for.
    #[test]
    fn template_names_translated() {
        let messages = parse_messages(Language::En.source());
        let asked = std::cell::RefCell::new(Vec::new());
        let project = cc_core::Project::from_template_localized(&|k| {
            asked.borrow_mut().push(k.to_string());
            messages.get(k).cloned()
        });
        let missing: Vec<_> = asked.borrow().iter().filter(|k| !messages.contains_key(*k)).cloned().collect();
        assert!(missing.is_empty(), "missing template names: {missing:?}");
        assert!(project.levels.iter().all(|l| !l.name.is_empty()));
        // Layers must reference materials that exist under their translated names.
        for t in project.types.values() {
            for layer in t.layers() {
                assert!(project.materials.iter().any(|m| m.name == layer.material), "{}", layer.material);
            }
        }
    }
}
