use std::borrow::Cow;

pub struct Translator {
    bundle: fluent::bundle::FluentBundle<
        &'static fluent::FluentResource,
        intl_memoizer::concurrent::IntlLangMemoizer,
    >,
    primary_language: unic_langid::LanguageIdentifier,
}
impl Translator {
    pub fn new(
        bundle: fluent::bundle::FluentBundle<
            &'static fluent::FluentResource,
            intl_memoizer::concurrent::IntlLangMemoizer,
        >,
        primary_language: unic_langid::LanguageIdentifier,
    ) -> Translator {
        Translator {
            bundle,
            primary_language,
        }
    }

    pub fn tr<'a>(&'a self, input: &'a LangKey) -> Cow<'a, str> {
        let LangKey(key, args) = input;
        let args = args.as_ref();

        let mut errors = Vec::with_capacity(0);
        let out = match self.bundle.get_message(key) {
            Some(msg) => self.bundle.format_pattern(
                msg.value().expect("Missing value for translation key"),
                args,
                &mut errors,
            ),
            None => {
                log::error!("Missing translation for {}", key);
                Cow::Borrowed(*key)
            }
        };
        if !errors.is_empty() {
            log::error!("Errors in translation: {:?}", errors);
        }

        out
    }

    pub fn primary_language(&self) -> &unic_langid::LanguageIdentifier {
        &self.primary_language
    }
}
impl std::fmt::Debug for Translator {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "Translator")
    }
}

pub struct LangKey<'a>(&'static str, Option<fluent::FluentArgs<'a>>);

#[allow(unused)]
pub mod keys {
    use super::*;

    include!(concat!(env!("OUT_DIR"), "/lang_keys.rs"));
}

pub use keys::*;
