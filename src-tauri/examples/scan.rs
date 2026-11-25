//! cargo run -p nasuaframe --example scan -- <shot.png | --screen SECONDS> [--lang pt-BR] [--raw] [--save out.png] [--items names.txt] [--full]
//! names.txt has one item per line, `name` or `name;english name`, used instead of the app catalog

#[cfg(windows)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::path::PathBuf;
    use std::time::{Duration, Instant};

    use nasua_core::model::{Item, Platform};
    use nasua_core::reward;
    use nasua_market::rest;
    use nasuaframe_lib::{scan, storage};

    let mut args = std::env::args().skip(1);
    let (mut file, mut screen, mut lang, mut raw, mut save, mut list, mut full) =
        (None, None, None, false, None, None, false);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--screen" => screen = Some(args.next().ok_or("--screen needs seconds")?.parse()?),
            "--lang" => lang = Some(args.next().ok_or("--lang needs a tag")?),
            "--raw" => raw = true,
            "--full" => full = true,
            "--save" => save = Some(args.next().ok_or("--save needs a path")?),
            "--items" => list = Some(args.next().ok_or("--items needs a path")?),
            flag if flag.starts_with("--") => return Err(format!("unknown option {flag}").into()),
            _ => file = Some(arg),
        }
    }

    println!("ocr languages: {:?}", scan::ocr::languages()?);

    let img = match (file, screen) {
        (Some(path), _) => image::open(path)?.to_rgba8(),
        (None, Some(secs)) => {
            println!("capturing in {secs}s");
            std::thread::sleep(Duration::from_secs(secs));
            let shot = scan::capture_primary()?;
            let name = format!(
                "shot-{}.png",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)?
                    .as_secs()
            );
            shot.save(&name)?;
            println!("saved {name}");
            shot
        }
        _ => return Err("pass a png or --screen SECONDS".into()),
    };

    let img = if full { img } else { scan::names_strip(&img) };
    let prepared = if raw {
        image::imageops::grayscale(&img)
    } else {
        scan::prepare(&img)
    };
    if let Some(path) = save {
        prepared.save(path)?;
    }

    let started = Instant::now();
    let words = scan::ocr::read(&prepared, lang.as_deref())?;
    println!(
        "ocr: {} words in {} ms",
        words.len(),
        started.elapsed().as_millis()
    );

    let mut line = usize::MAX;
    for word in &words {
        if word.line != line {
            line = word.line;
            print!("\n  ");
        }
        print!("{} ", word.text);
    }
    println!("\n");

    let mut items: Vec<Item> = match list {
        Some(path) => std::fs::read_to_string(path)?
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| {
                let (name, name_en) = l.split_once(';').unwrap_or((l, l));
                Item {
                    id: name_en.trim().to_string(),
                    slug: name_en.trim().to_lowercase().replace(' ', "_"),
                    name: name.trim().to_string(),
                    name_en: name_en.trim().to_string(),
                    max_rank: None,
                    subtypes: Vec::new(),
                    ducats: None,
                    game_ref: String::new(),
                }
            })
            .collect(),
        None => {
            let db = PathBuf::from(std::env::var("APPDATA")?)
                .join("io.github.gguedesa.nasuaframe")
                .join("nasuaframe.db");
            storage::items::load_all(&storage::open(&db)?)?
                .into_iter()
                .filter(reward::is_relic_reward)
                .collect()
        }
    };
    if items.is_empty() {
        println!("no catalog yet, open the app once or pass --items");
        return Ok(());
    }
    items.extend(reward::untradable());

    for card in reward::cards(&words) {
        if let Some((item, score)) = reward::best_match(&card, &items) {
            let mark = if score >= reward::MIN_SCORE {
                "ok"
            } else {
                "--"
            };
            println!(
                "{mark} {score:.2}  {card:?} -> {} ({})",
                item.name, item.name_en
            );
            if score < reward::MIN_SCORE {
                continue;
            }
            if item.slug.is_empty() {
                println!("          untradable");
                continue;
            }
            std::thread::sleep(Duration::from_millis(400));
            let price = match rest::fetch_top_orders(&item.slug, Platform::Pc, true, None, None) {
                Ok(orders) => reward::lowest_sell(&orders)
                    .map_or("no sellers online".to_string(), |p| format!("{p}p")),
                Err(e) => format!("price failed: {e}"),
            };
            let ducats = item.ducats.map_or("-".to_string(), |d| d.to_string());
            println!("          {price}, {ducats} ducats");
        }
    }
    Ok(())
}

#[cfg(not(windows))]
fn main() {
    eprintln!("windows only for now");
}
