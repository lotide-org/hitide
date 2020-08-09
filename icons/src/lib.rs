pub struct Icon {
    pub path: &'static str,
    pub content: &'static str,
}

macro_rules! icons_consts {
    ($i:ident => $p:expr) => {
        pub const $i: Icon = Icon {
            path: $p,
            content: include_str!(concat!("../res/", $p)),
        };
    };
    ($i1:ident => $p1:expr, $($i2:ident => $p2:expr),+) => {
        icons_consts! { $i1 => $p1 }
        icons_consts! { $($i2 => $p2),+ }
    }
}

macro_rules! icons_map {
    ($($i:ident => $p:expr),+) => {
        pub const ICONS_MAP: phf::Map<&'static str, &'static Icon> = phf::phf_map! {
            $($p => &icons::$i),+
        };
    }
}

macro_rules! icons {
    ($($i:ident => $p:expr),+) => {
        pub mod icons {
            use super::Icon;

            icons_consts! {
                $($i => $p),+
            }
        }

        icons_map! {
            $($i => $p),+
        }
    }
}

icons! {
    NOTIFICATIONS => "notifications.svg",
    NOTIFICATIONS_SOME => "notifications-some.svg",
    PERSON => "person.svg",
    UPVOTE => "upvote.svg",
    UPVOTED => "upvoted.svg"
}

pub use icons::*;
