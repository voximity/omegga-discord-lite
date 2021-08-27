use lazy_static::lazy_static;
use regex::Regex;

#[derive(Debug, Clone)]
pub struct Formatter {
    pub key: &'static str,
    pub value: String,
}

impl Formatter {
    pub fn format(&self, source: String) -> String {
        source.replace(&format!("${}", self.key), self.value.as_str())
    }
}

/// Compose a Vec<T> from a bunch of other Vec<T>s in one big Vec<Vec<T>>.
pub fn compose_vec<T>(vecs: Vec<Vec<T>>) -> Vec<T> {
    let mut vec = vec![];
    for v in vecs.into_iter() {
        vec.extend(v.into_iter());
    }
    vec
}

/// Format some text with the formatters provided.
pub fn format_content(text: String, formatters: &[Formatter]) -> String {
    let mut buf = text;
    for formatter in formatters.iter() {
        buf = formatter.format(buf);
    }
    buf
}

/// With a list of roles, determine the highest role list string.
pub fn role_text(roles: &[String], list: &[String]) -> String {
    let list = list
        .iter()
        .filter_map(|s| s.split_once(':'))
        .collect::<Vec<_>>();

    for role in roles.iter() {
        if let Some((_, m)) = list.iter().find(|(r, _)| r == role) {
            return String::from(*m);
        }
    }

    if let Some((_, m)) = list.iter().find(|(r, _)| *r == "default") {
        return String::from(*m);
    }

    String::new()
}

/// Convert a Discord MD formatted message to Brickadia's chat codes.
pub fn format_to_game(source: String) -> String {
    lazy_static! {
        static ref REGEXES: Vec<(Regex, &'static str)> = vec![
            (
                Regex::new("<:br_([A-Za-z_]+):\\d+>").unwrap(),
                "<emoji>$1</>"
            ),
            (Regex::new("<@!(\\d+)>").unwrap(), "<link=\"https://discord.com/users/$1\">@$1</>"),
            (Regex::new("`(.+)`").unwrap(), "<code>$1</>"),
            (Regex::new("\\*\\*(.+)\\*\\*").unwrap(), "<b>$1</>"),
            (Regex::new("\\*(.+)\\*").unwrap(), "<i>$1</>"),
            (Regex::new("__(.+)__").unwrap(), "<u>$1</>"),
            (Regex::new("_(.+)_").unwrap(), "<i>$1</>"),
        ];
    }

    let mut source = source;

    for (regex, replacement) in REGEXES.iter() {
        source = regex.replace_all(&source, *replacement).to_string();
    }

    source
}
