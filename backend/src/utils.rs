use rand::{distributions::Alphanumeric, thread_rng, Rng};

pub fn random_string(size: usize) -> std::string::String {
    thread_rng()
        .sample_iter(&Alphanumeric)
        .take(size)
        .map(char::from)
        .collect()
}

use serde::{de, Deserialize};
pub use trim_in_place::*;

pub fn string_trim<'de, D>(d: D) -> Result<String, D::Error>
where
    D: de::Deserializer<'de>,
{
    let mut de_string = String::deserialize(d)?;
    de_string.trim_in_place();
    Ok(de_string)
}

pub fn option_string_trim<'de, D>(d: D) -> Result<Option<String>, D::Error>
where
    D: de::Deserializer<'de>,
{
    let mut de_string: Option<String> = Option::deserialize(d)?;
    if let Some(ref mut de_string) = de_string {
        if de_string.trim_in_place().is_empty() {
            return Ok(None);
        }
    }
    Ok(de_string)
}

pub fn vec_trim_remove_empties<'de, D>(d: D) -> Result<Vec<String>, D::Error>
where
    D: de::Deserializer<'de>,
{
    let mut de_vec: Vec<String> = Vec::deserialize(d)?;
    de_vec = de_vec
        .iter_mut()
        .map(|s| s.trim_in_place().to_owned())
        .filter(|s| !s.is_empty())
        .collect();
    Ok(de_vec)
}

#[test]
fn test_string_trim() {
    #[derive(Deserialize)]
    struct Foo {
        #[serde(deserialize_with = "string_trim")]
        name: String,
    }
    let json = r#"{"name":" "}"#;
    let foo = serde_json::from_str::<Foo>(json).unwrap();
    assert_eq!(foo.name, "");
}

#[test]
fn test_option_string_trim() {
    #[derive(Deserialize)]
    struct OptionFoo {
        #[serde(deserialize_with = "option_string_trim")]
        name: Option<String>,
    }
    let json = r#"{"name":" "}"#;
    let foo = serde_json::from_str::<OptionFoo>(json).unwrap();
    assert_eq!(foo.name, None);

    #[derive(Deserialize)]
    struct OptionBar {
        #[serde(default, deserialize_with = "option_string_trim")]
        name: Option<String>,
        addr: String,
    }
    let json = r#"{"addr":"ABC"}"#;
    let foo = serde_json::from_str::<OptionBar>(json).unwrap();
    assert_eq!(foo.name, None);
    assert_eq!(foo.addr, "ABC");
}

#[test]
fn test_vec_trim_remove_empties() {
    #[derive(Deserialize)]
    struct Foo {
        #[serde(deserialize_with = "vec_trim_remove_empties")]
        names: Vec<String>,
    }
    let json = r#"{"names":["to_keep","  to_trim  ","","     "]}"#;
    let foo = serde_json::from_str::<Foo>(json).unwrap();
    assert!(foo.names.len() == 2);
    assert_eq!(foo.names[0], "to_keep");
    assert_eq!(foo.names[1], "to_trim");
}
