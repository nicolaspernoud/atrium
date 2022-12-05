use http::StatusCode;
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

pub fn option_vec_trim_remove_empties<'de, D>(d: D) -> Result<Option<Vec<String>>, D::Error>
where
    D: de::Deserializer<'de>,
{
    let de_vec: Option<Vec<String>> = Option::deserialize(d)?;
    Ok(de_vec.map(|mut e| {
        e.iter_mut()
            .map(|s| s.trim_in_place().to_owned())
            .filter(|s| !s.is_empty())
            .collect()
    }))
}

pub(crate) fn is_default<T: Default + PartialEq>(t: &T) -> bool {
    t == &T::default()
}

const QUERY_ERROR: (StatusCode, &str) = (StatusCode::INTERNAL_SERVER_ERROR, "query is empty");

pub fn raw_query_pairs<'a>(
    query: Option<&'a str>,
) -> Result<std::collections::HashMap<&'a str, &'a str>, (StatusCode, &'static str)> {
    let query = query.ok_or(QUERY_ERROR)?;
    if query.is_empty() {
        return Err(QUERY_ERROR);
    }
    let mut ooq = std::collections::HashMap::new();
    let query: Vec<&str> = query.split('&').collect();
    for keyvalue in query {
        let kv: Vec<&str> = keyvalue.split('=').collect();
        if kv.len() >= 2 && !kv[1].is_empty() {
            ooq.insert(kv[0], kv[1]);
        }
    }
    Ok(ooq)
}

#[cfg(test)]
mod tests {
    use crate::utils::{
        option_string_trim, option_vec_trim_remove_empties, raw_query_pairs, string_trim,
        vec_trim_remove_empties,
    };
    use serde::Deserialize;

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

    #[test]
    fn test_option_vec_trim_remove_empties() {
        #[derive(Deserialize)]
        struct Foo {
            #[serde(default, deserialize_with = "option_vec_trim_remove_empties")]
            names: Option<Vec<String>>,
        }
        let json = r#"{"names":["to_keep","  to_trim  ","","     "]}"#;
        let foo = serde_json::from_str::<Foo>(json).unwrap();
        let names = foo.names.unwrap();
        assert!(names.len() == 2);
        assert_eq!(names[0], "to_keep");
        assert_eq!(names[1], "to_trim");
        let json = r#"{}"#;
        let foo = serde_json::from_str::<Foo>(json).unwrap();
        assert!(foo.names.is_none());
    }

    #[test]
    fn test_query_pairs_ok() {
        let query = Some("a=1&b=2&c=3");
        let qp = raw_query_pairs(query).unwrap();
        assert_eq!(qp.get("a").unwrap(), &"1");
        assert_eq!(qp.get("c").unwrap(), &"3");
    }

    #[test]
    fn test_query_pairs_none() {
        let query = None;
        let qp = raw_query_pairs(query);
        assert!(qp.is_err());
    }

    #[test]
    fn test_query_pairs_empty() {
        let query = Some("");
        let qp = raw_query_pairs(query);
        assert!(qp.is_err());
    }

    #[test]
    fn test_query_pairs_empty_value() {
        let query = Some("a=1&b=2&c=");
        let qp = raw_query_pairs(query).unwrap();
        assert_eq!(qp.get("a").unwrap(), &"1");
        assert!(qp.get("c").is_none());
    }
}
