use crate::model::{Item, Order, OrderType};

pub const MIN_SCORE: f32 = 0.74;

#[derive(Debug, Clone, PartialEq)]
pub struct Word {
    pub text: String,
    pub line: usize,
    pub x: f32,
    pub width: f32,
    pub height: f32,
}

pub const UNTRADABLE: &[&str] = &[
    "Forma Blueprint",
    "2X Forma Blueprint",
    "Exilus Weapon Adapter Blueprint",
    "1200X Kuva",
    "Riven Sliver",
];

const NO_DUCATS: &[&str] = &[
    "Ayatan Amber Star",
    "Fass",
    "Jahu",
    "Khra",
    "Lohk",
    "Netra",
    "Ris",
    "Vome",
    "Xata",
];

pub fn is_relic_reward(item: &Item) -> bool {
    (item.ducats.is_some() && !item.name_en.ends_with(" Set"))
        || NO_DUCATS.contains(&item.name_en.as_str())
}

pub fn untradable() -> impl Iterator<Item = Item> {
    UNTRADABLE.iter().map(|name| Item {
        id: String::new(),
        slug: String::new(),
        name: name.to_string(),
        name_en: name.to_string(),
        max_rank: None,
        subtypes: Vec::new(),
        ducats: None,
        game_ref: String::new(),
    })
}

pub fn lowest_sell(orders: &[Order]) -> Option<u32> {
    orders
        .iter()
        .filter(|o| o.order_type == OrderType::Sell)
        .map(|o| o.platinum)
        .min()
}

pub fn cards(words: &[Word]) -> Vec<String> {
    let mut sorted: Vec<&Word> = words.iter().collect();
    sorted.sort_by(|a, b| a.line.cmp(&b.line).then(a.x.total_cmp(&b.x)));

    let mut segments: Vec<Segment> = Vec::new();
    for word in sorted {
        match segments.last_mut() {
            Some(seg) if seg.line == word.line && word.x <= seg.right + word.height * 0.8 => {
                seg.right = seg.right.max(word.x + word.width);
                seg.height = seg.height.max(word.height);
                seg.words.push(word);
            }
            _ => segments.push(Segment {
                line: word.line,
                left: word.x,
                right: word.x + word.width,
                height: word.height,
                words: vec![word],
            }),
        }
    }

    segments.sort_by(|a, b| a.center().total_cmp(&b.center()));
    let mut groups: Vec<Vec<Segment>> = Vec::new();
    for seg in segments {
        match groups.last_mut() {
            Some(group) if (seg.center() - group[0].center()).abs() < seg.height * 3.0 => {
                group.push(seg)
            }
            _ => groups.push(vec![seg]),
        }
    }

    groups
        .into_iter()
        .map(|mut group| {
            group.sort_by_key(|seg| seg.line);
            group
                .iter()
                .flat_map(|seg| seg.words.iter().map(|w| w.text.as_str()))
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect()
}

struct Segment<'a> {
    line: usize,
    left: f32,
    right: f32,
    height: f32,
    words: Vec<&'a Word>,
}

impl Segment<'_> {
    fn center(&self) -> f32 {
        (self.left + self.right) / 2.0
    }
}

pub fn best_match<'a>(text: &str, items: &'a [Item]) -> Option<(&'a Item, f32)> {
    let text = normalize(text);
    let words: Vec<&str> = text.split(' ').collect();
    items
        .iter()
        .map(|item| {
            let (name, name_en) = (normalize(&item.name), normalize(&item.name_en));
            let score = span_score(&words, &name).max(span_score(&words, &name_en));
            // ash prime systems also scores 1.0 inside ash prime systems blueprint
            let whole = similarity(&text, &name).max(similarity(&text, &name_en));
            (item, score, whole)
        })
        .max_by(|a, b| a.1.total_cmp(&b.1).then(a.2.total_cmp(&b.2)))
        .map(|(item, score, _)| (item, score))
}

fn span_score(words: &[&str], name: &str) -> f32 {
    let size = name.split(' ').count();
    let mut best = 0.0f32;
    for len in size.saturating_sub(1).max(1)..=size + 1 {
        for run in words.windows(len.min(words.len())) {
            best = best.max(similarity(&run.join(" "), name));
        }
    }
    best
}

fn normalize(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars().flat_map(char::to_lowercase) {
        let c = fold(c);
        if c.is_alphanumeric() {
            out.push(c);
        } else if !out.is_empty() && !out.ends_with(' ') {
            out.push(' ');
        }
    }
    out.trim_end().to_string()
}

fn fold(c: char) -> char {
    match c {
        'á' | 'à' | 'â' | 'ã' | 'ä' | 'å' | 'ą' => 'a',
        'é' | 'è' | 'ê' | 'ë' | 'ę' => 'e',
        'í' | 'ì' | 'î' | 'ï' => 'i',
        'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
        'ú' | 'ù' | 'û' | 'ü' => 'u',
        'ç' | 'ć' => 'c',
        'ñ' | 'ń' => 'n',
        'ł' => 'l',
        'ś' => 's',
        'ż' | 'ź' => 'z',
        _ => c,
    }
}

fn similarity(a: &str, b: &str) -> f32 {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let longest = a.len().max(b.len());
    if longest == 0 {
        return 0.0;
    }
    1.0 - levenshtein(&a, &b) as f32 / longest as f32
}

fn levenshtein(a: &[char], b: &[char]) -> usize {
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut diagonal = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let above = row[j + 1];
            row[j + 1] = if ca == cb {
                diagonal
            } else {
                1 + diagonal.min(above).min(row[j])
            };
            diagonal = above;
        }
    }
    row[b.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(name: &str, name_en: &str) -> Item {
        Item {
            id: name_en.to_string(),
            slug: name_en.to_lowercase().replace(' ', "_"),
            name: name.to_string(),
            name_en: name_en.to_string(),
            max_rank: None,
            subtypes: Vec::new(),
            ducats: None,
            game_ref: String::new(),
        }
    }

    fn word(text: &str, line: usize, x: f32) -> Word {
        Word {
            text: text.to_string(),
            line,
            x,
            width: text.len() as f32 * 10.0,
            height: 20.0,
        }
    }

    fn line(text: &str, line: usize, center: f32) -> Vec<Word> {
        let width = text.len() as f32 * 10.0;
        let mut x = center - width / 2.0;
        text.split(' ')
            .map(|t| {
                let w = word(t, line, x);
                x += w.width + 6.0;
                w
            })
            .collect()
    }

    #[test]
    fn normalize_drops_case_accents_and_symbols() {
        assert_eq!(normalize("  Cânone-PRIME: Cano!! "), "canone prime cano");
    }

    #[test]
    fn levenshtein_counts_edits() {
        let chars = |s: &str| s.chars().collect::<Vec<_>>();
        assert_eq!(levenshtein(&chars("kitten"), &chars("sitting")), 3);
        assert_eq!(levenshtein(&chars(""), &chars("abc")), 3);
        assert_eq!(levenshtein(&chars("same"), &chars("same")), 0);
    }

    #[test]
    fn relic_rewards_are_parts_with_ducats_and_requiem_mods() {
        let with_ducats = |name_en| Item {
            ducats: Some(45),
            ..item("x", name_en)
        };
        assert!(is_relic_reward(&with_ducats("Ash Prime Systems Blueprint")));
        assert!(!is_relic_reward(&with_ducats("Ash Prime Set")));
        assert!(is_relic_reward(&item("x", "Lohk")));
        assert!(!is_relic_reward(&item("x", "Primed Continuity")));
        assert!(!is_relic_reward(&item("x", "Neo A1 Relic")));
    }

    #[test]
    fn cards_keep_wrapped_lines_together() {
        let words = [
            line("Ash Prime", 0, 150.0),
            line("Systems Blueprint", 1, 152.0),
            line("Forma Blueprint", 0, 450.0),
        ]
        .concat();
        assert_eq!(
            cards(&words),
            ["Ash Prime Systems Blueprint", "Forma Blueprint"]
        );
    }

    #[test]
    fn cards_split_names_that_share_an_ocr_line() {
        let mut words = line("Stradavar Prime Barrel", 0, 760.0);
        words.extend(line("Lex Prime Blueprint", 0, 990.0));
        assert_eq!(
            cards(&words),
            ["Stradavar Prime Barrel", "Lex Prime Blueprint"]
        );
    }

    #[test]
    fn matches_game_language_or_english() {
        let items = [
            item("Cano de Akstiletto Prime", "Akstiletto Prime Barrel"),
            item("Sistemas de Ash Prime", "Ash Prime Systems"),
        ];

        let (found, score) = best_match("Cano de Akstilett0 Prime", &items).unwrap();
        assert_eq!(found.name_en, "Akstiletto Prime Barrel");
        assert!(score >= MIN_SCORE);

        let (found, score) = best_match("ASH PRIME SYSTEMS", &items).unwrap();
        assert_eq!(found.name_en, "Ash Prime Systems");
        assert_eq!(score, 1.0);
    }

    #[test]
    fn stray_words_around_the_name_are_ignored() {
        let items = [
            item("Lex Prime Blueprint", "Lex Prime Blueprint"),
            item("Lex Prime Barrel", "Lex Prime Barrel"),
        ];
        let (found, score) = best_match("3' Owned Lex Prime Blueprint", &items).unwrap();
        assert_eq!(found.name_en, "Lex Prime Blueprint");
        assert_eq!(score, 1.0);
    }

    #[test]
    fn longer_name_wins_a_tie() {
        for items in [
            [
                item("x", "Ash Prime Systems"),
                item("y", "Ash Prime Systems Blueprint"),
            ],
            [
                item("y", "Ash Prime Systems Blueprint"),
                item("x", "Ash Prime Systems"),
            ],
        ] {
            let (found, _) = best_match("Ash Prime Systems Blueprint", &items).unwrap();
            assert_eq!(found.name_en, "Ash Prime Systems Blueprint");
            let (found, _) = best_match("Ash Prime Systems", &items).unwrap();
            assert_eq!(found.name_en, "Ash Prime Systems");
        }
    }

    #[test]
    fn unrelated_text_scores_low() {
        let items = [item("Sistemas de Ash Prime", "Ash Prime Systems")];
        let (_, score) = best_match("Squad vote ends in 10", &items).unwrap();
        assert!(score < MIN_SCORE);
    }

    #[test]
    fn lowest_sell_skips_buy_orders() {
        use crate::model::{Platform, Status, Trader};
        let order = |order_type, platinum| Order {
            id: "id".into(),
            item_id: "item".into(),
            order_type,
            platinum,
            rank: None,
            subtype: None,
            user: Trader {
                ingame_name: "TestTrader".into(),
                platform: Platform::Pc,
                crossplay: true,
                status: Status::Ingame,
            },
        };
        let orders = [
            order(OrderType::Sell, 12),
            order(OrderType::Sell, 9),
            order(OrderType::Buy, 20),
        ];
        assert_eq!(lowest_sell(&orders), Some(9));
        assert_eq!(lowest_sell(&orders[2..]), None);
    }
}
