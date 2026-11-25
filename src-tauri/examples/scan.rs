//! cargo run -p nasuaframe --example scan -- <shot.png | --screen SECONDS | --watch> [--lang pt-BR] [--raw] [--save out.png] [--items names.txt] [--full]
//! names.txt has one item per line, `name` or `name;english name`, used instead of the app catalog

#[cfg(windows)]
use nasua_core::model::Item;

#[cfg(windows)]
struct Opts {
    lang: Option<String>,
    raw: bool,
    full: bool,
    save: Option<String>,
}

#[cfg(windows)]
type Res<T> = Result<T, Box<dyn std::error::Error>>;

#[cfg(windows)]
fn main() -> Res<()> {
    use std::time::Duration;

    let mut args = std::env::args().skip(1);
    let (mut file, mut screen, mut watch, mut list) = (None, None, false, None);
    let mut opts = Opts {
        lang: None,
        raw: false,
        full: false,
        save: None,
    };
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--screen" => screen = Some(args.next().ok_or("--screen needs seconds")?.parse()?),
            "--watch" => watch = true,
            "--lang" => opts.lang = Some(args.next().ok_or("--lang needs a tag")?),
            "--raw" => opts.raw = true,
            "--full" => opts.full = true,
            "--save" => opts.save = Some(args.next().ok_or("--save needs a path")?),
            "--items" => list = Some(args.next().ok_or("--items needs a path")?),
            flag if flag.starts_with("--") => return Err(format!("unknown option {flag}").into()),
            _ => file = Some(arg),
        }
    }

    println!(
        "ocr languages: {:?}",
        nasuaframe_lib::scan::ocr::languages()?
    );
    let items = load_items(list)?;
    if items.is_empty() {
        println!("no catalog yet, open the app once or pass --items");
        return Ok(());
    }

    if watch {
        return watch_log(&opts, &items);
    }
    let img = match (file, screen) {
        (Some(path), _) => image::open(path)?.to_rgba8(),
        (None, Some(secs)) => {
            println!("capturing in {secs}s");
            std::thread::sleep(Duration::from_secs(secs));
            capture()?
        }
        _ => return Err("pass a png, --screen SECONDS or --watch".into()),
    };
    scan(&img, &opts, &items, None)
}

#[cfg(windows)]
fn load_items(list: Option<String>) -> Res<Vec<Item>> {
    use nasua_core::reward;
    use nasuaframe_lib::storage;

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
            let db = std::path::PathBuf::from(std::env::var("APPDATA")?)
                .join("io.github.gguedesa.nasuaframe")
                .join("nasuaframe.db");
            storage::items::load_all(&storage::open(&db)?)?
                .into_iter()
                .filter(reward::is_relic_reward)
                .collect()
        }
    };
    if !items.is_empty() {
        items.extend(reward::untradable());
    }
    Ok(items)
}

#[cfg(windows)]
fn capture() -> Res<image::RgbaImage> {
    let shot = nasuaframe_lib::scan::capture_primary()?;
    let name = format!(
        "shot-{}.png",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs()
    );
    shot.save(&name)?;
    println!("saved {name}");
    Ok(shot)
}

#[cfg(windows)]
fn watch_log(opts: &Opts, items: &[Item]) -> Res<()> {
    use nasua_core::reward::{self, LogEvent};
    use std::io::{Read, Seek, SeekFrom};
    use std::time::Duration;

    let path = std::path::PathBuf::from(std::env::var("LOCALAPPDATA")?)
        .join("Warframe")
        .join("EE.log");
    let size = || std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
    println!("watching {}", path.display());

    let mut pos = size();
    let mut pending = Vec::new();
    let mut own = None;
    loop {
        let len = size();
        if len < pos {
            pos = 0;
            pending.clear();
        }
        if len > pos {
            let mut file = std::fs::File::open(&path)?;
            file.seek(SeekFrom::Start(pos))?;
            file.take(len - pos).read_to_end(&mut pending)?;
            pos = len;
        }
        while let Some(end) = pending.iter().position(|&b| b == b'\n') {
            let line: Vec<u8> = pending.drain(..=end).collect();
            match reward::log_event(&String::from_utf8_lossy(&line)) {
                Some(LogEvent::OwnReward(game_ref)) => own = Some(game_ref),
                Some(LogEvent::GotRewards) => {
                    std::thread::sleep(Duration::from_millis(650));
                    let result = capture().and_then(|img| scan(&img, opts, items, own.as_deref()));
                    if let Err(e) = result {
                        eprintln!("scan failed: {e}");
                    }
                    own = None;
                }
                Some(LogEvent::Closed) => println!("reward screen closed\n"),
                None => {}
            }
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}

#[cfg(windows)]
fn scan(img: &image::RgbaImage, opts: &Opts, items: &[Item], own: Option<&str>) -> Res<()> {
    use nasua_core::reward;
    use nasuaframe_lib::scan;
    use std::time::Instant;

    let img = if opts.full {
        img.clone()
    } else {
        scan::names_strip(img)
    };
    let prepared = if opts.raw {
        image::imageops::grayscale(&img)
    } else {
        scan::prepare(&img)
    };
    if let Some(path) = &opts.save {
        prepared.save(path)?;
    }

    let started = Instant::now();
    let words = scan::ocr::read(&prepared, opts.lang.as_deref())?;
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

    let mut own = own;
    for card in reward::cards(&words) {
        let Some((item, score)) = reward::best_match(&card, items) else {
            continue;
        };
        let found = score >= reward::MIN_SCORE;
        let mine = found && own == Some(item.game_ref.as_str());
        if mine {
            own = None;
        }
        let mark = match (mine, found) {
            (true, _) => "me",
            (_, true) => "ok",
            _ => "--",
        };
        println!(
            "{mark} {score:.2}  {card:?} -> {} ({})",
            item.name, item.name_en
        );
        if found {
            show_price(item);
        }
    }

    if let Some(game_ref) = own {
        match items.iter().find(|i| i.game_ref == game_ref) {
            Some(item) => {
                println!("me  not read -> {} ({})", item.name, item.name_en);
                show_price(item);
            }
            None => println!("me  not read -> {game_ref}"),
        }
    }
    Ok(())
}

#[cfg(windows)]
fn show_price(item: &Item) {
    use nasua_core::model::Platform;
    use nasua_core::reward;
    use nasua_market::rest;

    if item.slug.is_empty() {
        println!("          untradable");
        return;
    }
    std::thread::sleep(std::time::Duration::from_millis(400));
    let price = match rest::fetch_top_orders(&item.slug, Platform::Pc, true, None, None) {
        Ok(orders) => reward::lowest_sell(&orders)
            .map_or("no sellers online".to_string(), |p| format!("{p}p")),
        Err(e) => format!("price failed: {e}"),
    };
    let ducats = item.ducats.map_or("-".to_string(), |d| d.to_string());
    println!("          {price}, {ducats} ducats");
}

#[cfg(not(windows))]
fn main() {
    eprintln!("windows only for now");
}
