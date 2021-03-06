pub struct Icon {
    pub path: &'static str,
    pub content: &'static str,
    pub dark_invert: bool,
}

mod icons {
    include!(concat!(env!("OUT_DIR"), "/icons.rs"));
}
pub use icons::*;
