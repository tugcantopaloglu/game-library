use crate::media;
use library_core::models::{CompletionTimes, Game, Source};
use serde_json::Value;
pub struct FetchedCompletion {
    pub times: CompletionTimes,
    pub image_url: Option<String>,
}

pub fn page_id(value: &str) -> Result<u32, String> {
    if value.bytes().all(|byte| byte.is_ascii_digit()) {
        return value
            .parse::<u32>()
            .ok()
            .filter(|id| *id > 0)
            .ok_or("HowLongToBeat oyun kimliği geçersiz".into());
    }
    let url = media::trusted_url(value, &["howlongtobeat.com", "www.howlongtobeat.com"])?;
    let id = url.path().strip_prefix("/game/").or_else(|| {
        if url.path() == "/game" {
            None
        } else {
            Some("")
        }
    });
    let id = id
        .map(str::to_owned)
        .or_else(|| {
            url.query_pairs()
                .find(|(key, _)| key == "id")
                .map(|(_, value)| value.into_owned())
        })
        .ok_or("HowLongToBeat oyun sayfasını seç.")?;
    id.parse::<u32>()
        .ok()
        .filter(|id| *id > 0)
        .ok_or("HowLongToBeat oyun sayfasını seç.".into())
}

pub fn parse_page(html: &str, expected: u32) -> Result<(FetchedCompletion, u32), String> {
    let marker = "<script id=\"__NEXT_DATA__\" type=\"application/json\">";
    let json = html
        .split_once(marker)
        .and_then(|(_, rest)| rest.split_once("</script>"))
        .map(|(value, _)| value)
        .ok_or("HowLongToBeat sayfa biçimi değişmiş veya erişilemiyor")?;
    let data: Value =
        serde_json::from_str(json).map_err(|_| "HowLongToBeat oyun verisi okunamadı")?;
    let game = &data["props"]["pageProps"]["game"]["data"]["game"][0];
    if game["game_id"].as_u64() != Some(u64::from(expected)) {
        return Err("HowLongToBeat oyun kimliği uyuşmuyor".into());
    }
    let seconds = |field: &str| {
        game[field]
            .as_u64()
            .filter(|seconds| *seconds > 0 && *seconds <= 36_000_000)
            .map(|seconds| seconds as u32)
    };
    let name = game["game_name"]
        .as_str()
        .filter(|name| !name.is_empty() && name.len() <= 512)
        .ok_or("HowLongToBeat oyun adı eksik")?;
    let image_url = game["game_image"]
        .as_str()
        .filter(|file| !file.is_empty() && file.len() <= 512 && !file.contains(['/', '\\', '\0']))
        .and_then(|file| {
            let mut url = url::Url::parse("https://howlongtobeat.com/games/").ok()?;
            url.path_segments_mut().ok()?.pop_if_empty().push(file);
            Some(url.to_string())
        });
    Ok((
        FetchedCompletion {
            image_url,
            times: CompletionTimes {
                hltb_id: expected,
                name: name.into(),
                main_seconds: seconds("comp_main"),
                extra_seconds: seconds("comp_plus"),
                completionist_seconds: seconds("comp_100"),
                fetched_ms: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_err(|error| error.to_string())?
                    .as_millis() as i64,
            },
        },
        game["profile_steam"].as_u64().unwrap_or(0) as u32,
    ))
}

pub fn lookup(game: &Game, selected: &str) -> Result<FetchedCompletion, String> {
    let manual = !selected.trim().is_empty();
    let id = if manual {
        page_id(selected.trim())?
    } else {
        let query = if game.source == Source::Steam {
            format!("SELECT DISTINCT ?hltb WHERE {{ ?item wdt:P1733 \"{}\"; wdt:P2816 ?hltb. }} LIMIT 2",game.app_id)
        } else {
            let name = if game.source == Source::Riot {
                game.name.strip_suffix(" (pbe)").unwrap_or(&game.name)
            } else {
                &game.name
            };
            let titled = name
                .split_whitespace()
                .map(|word| {
                    let mut chars = word.chars();
                    chars
                        .next()
                        .map(|first| {
                            format!("{}{}", first.to_uppercase(), chars.as_str().to_lowercase())
                        })
                        .unwrap_or_default()
                })
                .collect::<Vec<_>>()
                .join(" ");
            let mut labels = std::collections::BTreeSet::new();
            for variant in [name, titled.as_str()] {
                let literal = serde_json::to_string(variant).map_err(|error| error.to_string())?;
                labels.insert(format!("{literal}@en"));
                labels.insert(format!("{literal}@mul"));
            }
            format!("SELECT DISTINCT ?hltb WHERE {{ VALUES ?label {{ {} }} ?item rdfs:label ?label; wdt:P2816 ?hltb. }} LIMIT 2",labels.into_iter().collect::<Vec<_>>().join(" "))
        };
        let mut url = url::Url::parse("https://query.wikidata.org/sparql")
            .map_err(|error| error.to_string())?;
        url.query_pairs_mut()
            .append_pair("format", "json")
            .append_pair("query", &query);
        let bytes = media::download(url.as_str(), &["query.wikidata.org"], 64 * 1024)?;
        let data: Value =
            serde_json::from_slice(&bytes).map_err(|_| "Oyun eşleştirme verisi okunamadı")?;
        let rows = data["results"]["bindings"]
            .as_array()
            .filter(|rows| rows.len() == 1)
            .ok_or(
                "Tek bir HowLongToBeat eşleşmesi bulunamadı. Siteden oyun sayfasını seçebilirsin.",
            )?;
        page_id(
            rows[0]["hltb"]["value"]
                .as_str()
                .ok_or("HowLongToBeat kimliği eksik")?,
        )?
    };
    let bytes = media::download(
        &format!("https://howlongtobeat.com/game/{id}"),
        &["howlongtobeat.com", "www.howlongtobeat.com"],
        2 * 1024 * 1024,
    )?;
    let html = std::str::from_utf8(&bytes).map_err(|_| "HowLongToBeat sayfası UTF-8 değil")?;
    let (completion, steam_id) = parse_page(html, id)?;
    if !manual && game.source == Source::Steam && steam_id != game.app_id {
        return Err("HowLongToBeat Steam kimliği uyuşmuyor; eşleşmeyi elle seç.".into());
    }
    Ok(completion)
}

pub fn summary(completion: &CompletionTimes) -> String {
    let hours = |seconds: Option<u32>| {
        seconds
            .map(|seconds| format!("{:.1} saat", f64::from(seconds) / 3600.0))
            .unwrap_or_else(|| "Veri yok".into())
    };
    format!(
        "{} · HowLongToBeat\nAna hikâye: {} · Ana + ek içerik: {} · Tamamlama: {}",
        completion.name,
        hours(completion.main_seconds),
        hours(completion.extra_seconds),
        hours(completion.completionist_seconds)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn completion_requires_a_real_game_page_and_preserves_missing_times() {
        for value in [
            "https://evil.test/game/42",
            "https://howlongtobeat.com.evil.test/game/42",
            "https://howlongtobeat.com/forum/42",
            "0",
        ] {
            assert!(page_id(value).is_err());
        }
        assert_eq!(page_id("https://howlongtobeat.com/game/42").unwrap(), 42);
        assert_eq!(page_id("https://howlongtobeat.com/game?id=42").unwrap(), 42);
        let html = r#"<script id="__NEXT_DATA__" type="application/json">{"props":{"pageProps":{"game":{"data":{"game":[{"game_id":42,"game_name":"Fixture","profile_steam":570,"comp_main":3600,"comp_plus":0,"comp_100":7200}]}}}}}</script>"#;
        let (completion, steam) = parse_page(html, 42).unwrap();
        assert_eq!(steam, 570);
        assert_eq!(completion.times.main_seconds, Some(3600));
        assert_eq!(completion.times.extra_seconds, None);
        assert!(parse_page(html, 41).is_err());
        assert!(parse_page("Access Denied", 42).is_err());
    }
}
