use std::hash::{Hash, Hasher};
use std::io::Write;

fn main() {
    println!("cargo:rerun-if-changed=res");
    let mut file = std::fs::File::create(format!(
        "{}{}icons.rs",
        std::env::var("OUT_DIR").unwrap(),
        std::path::MAIN_SEPARATOR
    ))
    .unwrap();
    let mut mapping = Vec::new();

    writeln!(file, "use super::Icon;").unwrap();

    for res in std::fs::read_dir("res").unwrap() {
        let path = res.unwrap().path();
        if path.is_dir() {
            continue;
        }

        println!("{:?}", path);

        let content = std::fs::read_to_string(&path).unwrap();

        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        content.hash(&mut hasher);
        let hash = hasher.finish();

        let key = format!("{}.svg", hash);

        let name = path.file_name().unwrap().to_str().unwrap();
        let name = (&name[0..name.len() - 4]).to_ascii_uppercase();

        let dark_invert = content.contains("<!--#darkInvert#-->");

        writeln!(
            file,
            "pub const {}: Icon=Icon{{path:\"{}\",content:include_str!(r#\"{}\"#),dark_invert:{}}};",
            name,
            key,
            path.canonicalize().unwrap().to_str().unwrap(),
            dark_invert,
        )
        .unwrap();

        mapping.push((key, name));
    }

    writeln!(
        file,
        "pub const ICONS_MAP: phf::Map<&'static str, &'static Icon> = phf::phf_map! {{"
    )
    .unwrap();
    for (key, name) in mapping {
        writeln!(file, "\"{}\" => &{},", key, name).unwrap();
    }
    writeln!(file, "}};").unwrap();
}
