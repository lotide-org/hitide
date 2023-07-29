use std::borrow::Cow;
use std::convert::TryFrom;

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

pub const PLACEHOLDER_BASE: u32 = '\u{fba00}' as u32;
pub const PLACEHOLDER_MAX: u32 = PLACEHOLDER_BASE + (u8::MAX as u32);

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct LangPlaceholder(pub u8);

impl fluent::types::FluentType for LangPlaceholder {
    fn duplicate(&self) -> Box<dyn fluent::types::FluentType + Send + 'static> {
        Box::new(*self)
    }

    fn as_string(&self, _intls: &intl_memoizer::IntlLangMemoizer) -> Cow<'static, str> {
        char::try_from(PLACEHOLDER_BASE + u32::from(self.0))
            .unwrap()
            .to_string()
            .into()
    }

    fn as_string_threadsafe(
        &self,
        _intls: &intl_memoizer::concurrent::IntlLangMemoizer,
    ) -> Cow<'static, str> {
        char::try_from(PLACEHOLDER_BASE + u32::from(self.0))
            .unwrap()
            .to_string()
            .into()
    }
}

impl From<LangPlaceholder> for fluent::FluentValue<'_> {
    fn from(src: LangPlaceholder) -> fluent::FluentValue<'static> {
        fluent::FluentValue::Custom(Box::new(src))
    }
}

pub struct TrElements<'a, F: (Fn(u8, &mut dyn std::fmt::Write) -> std::fmt::Result)> {
    src: Cow<'a, str>,
    render_placeholder: F,
}

impl<'a, F: (Fn(u8, &mut dyn std::fmt::Write) -> std::fmt::Result)> TrElements<'a, F> {
    pub fn new(src: Cow<'a, str>, render_placeholder: F) -> Self {
        Self {
            src,
            render_placeholder,
        }
    }
}

impl<'a, F: (Fn(u8, &mut dyn std::fmt::Write) -> std::fmt::Result)> render::Render
    for TrElements<'a, F>
{
    fn render_into<W: std::fmt::Write + ?Sized>(self, mut writer: &mut W) -> std::fmt::Result {
        let mut covered = 0;

        for (idx, chr) in self.src.char_indices() {
            let chr_value: u32 = chr.into();
            if chr_value >= PLACEHOLDER_BASE && chr_value <= PLACEHOLDER_MAX {
                if idx > covered {
                    self.src[covered..idx].render_into(writer)?;
                }

                (self.render_placeholder)((chr_value - PLACEHOLDER_BASE) as u8, &mut writer)?;

                covered = idx + chr.len_utf8();
            }
        }

        if covered < self.src.len() {
            self.src[covered..].render_into(writer)?;
        }

        Ok(())
    }
}

#[allow(unused)]
pub mod keys {
    use super::*;

    include!(concat!(env!("OUT_DIR"), "/lang_keys.rs"));
}

pub use keys::*;
