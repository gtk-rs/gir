use super::ident::Ident;
use toml::Value;
use version::Version;

#[derive(Clone, Debug)]
pub struct Member {
    pub ident: Ident,
    // some enum variants have multiple names
    pub alias: bool,
    pub version: Option<Version>,
}

impl Member {
    pub fn parse(toml: &Value, object_name: &str) -> Option<Member> {
        let ident = match Ident::parse(toml, object_name, "member") {
            Some(ident) => ident,
            None => {
                error!("No 'name' or 'pattern' given for member for object {}", object_name);
                return None
            }
        };
        let alias = toml.lookup("alias")
            .and_then(|val| val.as_bool())
            .unwrap_or(false);
        let version = toml.lookup("version")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse().ok());

        Some(Member{
            ident: ident,
            alias: alias,
            version: version,
        })
    }
}

#[derive(Clone, Debug)]
pub struct Members(Vec<Member>);

impl Members {
    pub fn new() -> Members {
        Members(Vec::new())
    }

    pub fn parse(toml: Option<&Value>, object_name: &str) -> Members {
        let mut v = Vec::new();
        if let Some(items) = toml.and_then(|val| val.as_slice()) {
            for item in items {
                if let Some(item) = Member::parse(item, object_name) {
                    v.push(item);
                }
            }
        }

        Members(v)
    }

    pub fn matched(&self, member_name: &str) -> Vec<&Member> {
        self.0.iter().filter(|m| m.ident.is_match(member_name)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::super::ident::Ident;
    use super::*;
    use toml;
    use version::Version;

    fn toml(input: &str) -> toml::Value {
        let mut parser = toml::Parser::new(&input);
        let value = parser.parse();
        assert!(value.is_some());
        toml::Value::Table(value.unwrap())
    }

    #[test]
    fn member_parse_alias() {
        let toml = toml(r#"
name = "name1"
alias = true
"#);
        let f = Member::parse(&toml, "a").unwrap();
        assert_eq!(f.ident, Ident::Name("name1".into()));
        assert_eq!(f.alias, true);
    }

    #[test]
    fn member_parse_version_default() {
        let toml = toml(r#"
name = "name1"
"#);
        let f = Member::parse(&toml, "a").unwrap();
        assert_eq!(f.version, None);
    }

    #[test]
    fn member_parse_version() {
        let toml = toml(r#"
name = "name1"
version = "3.20"
"#);
        let f = Member::parse(&toml, "a").unwrap();
        assert_eq!(f.version, Some(Version(3, 20, 0)));
    }
}
