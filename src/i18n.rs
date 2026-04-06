use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::FromRequestParts;
use axum::http::{header, request::Parts};
use minijinja::value::Kwargs;
use minijinja::{Environment, Error as JinjaError, State, Value};

/// Supported locale codes.
pub const SUPPORTED_LOCALES: &[&str] = &["en", "zh-TW", "zh-CN"];

/// Server-side internationalization state.
///
/// Holds translation maps for all supported locales, loaded from embedded JSON
/// files at startup. Thread-safe via `Arc` sharing.
pub struct I18n {
    /// locale code → (translation key → translated string)
    locales: HashMap<String, HashMap<String, String>>,
}

impl I18n {
    /// Load all embedded locale JSON files and build translation maps.
    pub fn load() -> Self {
        let en: HashMap<String, String> = serde_json::from_str(include_str!("../locales/en.json"))
            .expect("failed to parse locales/en.json");
        let zh_tw: HashMap<String, String> =
            serde_json::from_str(include_str!("../locales/zh-TW.json"))
                .expect("failed to parse locales/zh-TW.json");
        let zh_cn: HashMap<String, String> =
            serde_json::from_str(include_str!("../locales/zh-CN.json"))
                .expect("failed to parse locales/zh-CN.json");

        let mut locales = HashMap::with_capacity(3);
        locales.insert("en".into(), en);
        locales.insert("zh-TW".into(), zh_tw);
        locales.insert("zh-CN".into(), zh_cn);

        Self { locales }
    }

    /// Look up a translation key in the given locale, falling back to `en`.
    /// Returns the key itself if not found in any locale.
    fn get_text(&self, locale: &str, key: &str) -> String {
        self.locales
            .get(locale)
            .and_then(|m| m.get(key))
            .or_else(|| self.locales.get("en").and_then(|m| m.get(key)))
            .cloned()
            .unwrap_or_else(|| key.to_string())
    }

    /// Translate a key with named variable substitution.
    /// Replaces `{name}` placeholders with matching entries from `vars`.
    pub fn translate(&self, locale: &str, key: &str, vars: &[(&str, &str)]) -> String {
        let text = self.get_text(locale, key);
        let mut result = text;
        for (name, value) in vars {
            result = result.replace(&format!("{{{name}}}"), value);
        }
        result
    }

    /// Return all JS-facing translation keys (those starting with `js.`) for
    /// the given locale, with the `js.` prefix stripped. Falls back to `en`
    /// for any missing keys.
    pub fn js_translations(&self, locale: &str) -> HashMap<String, String> {
        let en = self.locales.get("en");
        let target = self.locales.get(locale);

        let Some(en_map) = en else {
            return HashMap::new();
        };

        let mut result = HashMap::new();
        for (key, en_val) in en_map {
            if let Some(stripped) = key.strip_prefix("js.") {
                let value = target.and_then(|m| m.get(key)).unwrap_or(en_val).clone();
                result.insert(stripped.to_string(), value);
            }
        }
        result
    }
}

// ── MiniJinja integration ────────────────────────────────────

/// Register the `t(key, **kwargs)` global function on the MiniJinja environment.
///
/// The function reads the `lang` variable from the current template context,
/// looks up the translation in the captured `I18n` instance, interpolates any
/// `{var}` placeholders with values from kwargs, and returns the result.
pub fn register_translate_function(env: &mut Environment<'static>, i18n: Arc<I18n>) {
    env.add_function(
        "t",
        move |state: &State, key: String, kwargs: Kwargs| -> Result<String, JinjaError> {
            let lang_value = state.lookup("lang");
            let lang = lang_value.as_ref().and_then(|v| v.as_str()).unwrap_or("en");

            let text = i18n.get_text(lang, &key);
            let result = interpolate_kwargs(&text, &kwargs);

            // Inform MiniJinja that all kwargs were consumed (prevents
            // "unknown keyword argument" warnings).
            kwargs.assert_all_used()?;

            Ok(result)
        },
    );
}

/// Replace `{var_name}` placeholders in `text` with values from `kwargs`.
/// Unknown variables are left as-is.
fn interpolate_kwargs(text: &str, kwargs: &Kwargs) -> String {
    let mut result = String::with_capacity(text.len());
    let mut rest = text;

    while let Some(open) = rest.find('{') {
        result.push_str(&rest[..open]);
        let after_open = &rest[open + 1..];

        if let Some(close) = after_open.find('}') {
            let var_name = &after_open[..close];
            // Try to resolve the variable from kwargs
            match kwargs.get::<Value>(var_name) {
                Ok(val) => result.push_str(&val.to_string()),
                Err(_) => {
                    // Not in kwargs — keep the placeholder
                    result.push('{');
                    result.push_str(var_name);
                    result.push('}');
                }
            }
            rest = &after_open[close + 1..];
        } else {
            // No closing brace — emit `{` and continue
            result.push('{');
            rest = after_open;
        }
    }
    result.push_str(rest);
    result
}

// ── Axum extractor ───────────────────────────────────────────

/// Axum extractor that resolves the active UI language from:
/// 1. `lang` cookie (explicit user choice)
/// 2. `Accept-Language` header (browser preference)
/// 3. Default `"en"`
pub struct Lang(pub String);

impl<S: Send + Sync> FromRequestParts<S> for Lang {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        // 1. Check lang cookie
        if let Some(cookie_header) = parts.headers.get(header::COOKIE)
            && let Ok(cookies) = cookie_header.to_str()
        {
            for pair in cookies.split(';') {
                let pair = pair.trim();
                if let Some(value) = pair.strip_prefix("lang=") {
                    let lang = value.trim();
                    if SUPPORTED_LOCALES.contains(&lang) {
                        return Ok(Lang(lang.to_string()));
                    }
                }
            }
        }

        // 2. Check Accept-Language header
        if let Some(accept) = parts.headers.get(header::ACCEPT_LANGUAGE)
            && let Ok(accept_str) = accept.to_str()
            && let Some(lang) = parse_accept_language(accept_str)
        {
            return Ok(Lang(lang));
        }

        // 3. Default
        Ok(Lang("en".to_string()))
    }
}

/// Parse an `Accept-Language` header value and return the best matching
/// supported locale, or `None` if no match is found.
fn parse_accept_language(header: &str) -> Option<String> {
    let mut entries: Vec<(&str, f32)> = header
        .split(',')
        .filter_map(|entry| {
            let entry = entry.trim();
            let (tag, quality) = if let Some((t, q)) = entry.split_once(";q=") {
                (t.trim(), q.trim().parse::<f32>().unwrap_or(0.0))
            } else {
                (entry, 1.0)
            };
            if tag.is_empty() {
                None
            } else {
                Some((tag, quality))
            }
        })
        .collect();

    // Sort by quality descending (stable sort preserves original order for ties)
    entries.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    for (tag, _) in entries {
        if let Some(locale) = match_locale(tag) {
            return Some(locale);
        }
    }

    None
}

/// Match a single language tag (e.g. `"zh-Hant"`, `"en-US"`) to a supported
/// locale code. Returns `None` for unsupported languages.
fn match_locale(tag: &str) -> Option<String> {
    let tag_lower = tag.to_lowercase();

    // Specific Chinese variant matching (order matters: Hant before Hans before bare zh)
    if tag_lower.starts_with("zh-hant") || tag_lower == "zh-tw" {
        return Some("zh-TW".to_string());
    }
    if tag_lower.starts_with("zh-hans") || tag_lower == "zh-cn" {
        return Some("zh-CN".to_string());
    }
    // Bare "zh" or any other zh-* variant defaults to Simplified
    if tag_lower.starts_with("zh") {
        return Some("zh-CN".to_string());
    }
    if tag_lower.starts_with("en") {
        return Some("en".to_string());
    }

    None
}

// ── Tests ────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accept_language_zh_tw() {
        let result = parse_accept_language("zh-TW,zh;q=0.9,en;q=0.8");
        assert_eq!(result, Some("zh-TW".to_string()));
    }

    #[test]
    fn accept_language_zh_hant() {
        let result = parse_accept_language("zh-Hant,en;q=0.5");
        assert_eq!(result, Some("zh-TW".to_string()));
    }

    #[test]
    fn accept_language_zh_hans() {
        let result = parse_accept_language("zh-Hans-CN,zh;q=0.9");
        assert_eq!(result, Some("zh-CN".to_string()));
    }

    #[test]
    fn accept_language_bare_zh_defaults_cn() {
        let result = parse_accept_language("zh");
        assert_eq!(result, Some("zh-CN".to_string()));
    }

    #[test]
    fn accept_language_english() {
        let result = parse_accept_language("en-US,en;q=0.9");
        assert_eq!(result, Some("en".to_string()));
    }

    #[test]
    fn accept_language_unsupported_falls_through() {
        let result = parse_accept_language("ja,ko;q=0.9");
        assert_eq!(result, None);
    }

    #[test]
    fn accept_language_quality_ordering() {
        // English has higher quality than Chinese here
        let result = parse_accept_language("zh-TW;q=0.5,en;q=0.9");
        assert_eq!(result, Some("en".to_string()));
    }

    #[test]
    fn match_locale_variants() {
        assert_eq!(match_locale("zh-Hant-TW"), Some("zh-TW".to_string()));
        assert_eq!(match_locale("zh-Hans-CN"), Some("zh-CN".to_string()));
        assert_eq!(match_locale("zh-SG"), Some("zh-CN".to_string()));
        assert_eq!(match_locale("en-GB"), Some("en".to_string()));
        assert_eq!(match_locale("fr"), None);
    }

    #[test]
    fn locale_key_consistency() {
        let i18n = I18n::load();
        let en_keys: std::collections::HashSet<_> = i18n.locales["en"].keys().collect();

        for locale in &["zh-TW", "zh-CN"] {
            let locale_keys: std::collections::HashSet<_> = i18n.locales[*locale].keys().collect();

            let missing: Vec<_> = en_keys.difference(&locale_keys).collect();
            assert!(
                missing.is_empty(),
                "Locale {locale} is missing keys: {missing:?}"
            );
        }
    }
}
