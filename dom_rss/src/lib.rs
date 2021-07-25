use chrono::Local;
use dom_api::*;

pub const FETCHING_BACKEND: &dyn FetchingBackend = &RssAtom {};

struct RssAtom {}

impl FetchingBackend for RssAtom {
    fn fetch_feed(&self, url: &str) -> Result<Vec<Episode>, anyhow::Error> {
        let response = minreq::get(url).send()?;
        let feed = feed_rs::parser::parse_with_uri(response.as_bytes(), Some(url))?;
        let podcast = feed
            .title
            .map_or_else(|| "".to_string(), |title| title.content);
        let mut eps: Vec<Episode> = feed
            .entries
            .into_iter()
            .flat_map(move |interep| {
                let published_at_base = interep
                    .published
                    .map_or_else(Local::now, |dt| dt.with_timezone(&Local));
                let title = interep
                    .title
                    .map_or_else(|| podcast.clone(), |title| title.content);
                let description = interep
                    .summary
                    .map_or_else(|| "".to_string(), |summary| summary.content);
                let podcast = podcast.clone();
                interep
                    .media
                    .into_iter()
                    .enumerate()
                    .map(move |(i, media)| Episode {
                        podcast: podcast.clone(),
                        published_at: published_at_base + chrono::Duration::nanoseconds(i as i64),
                        title: title.clone(),
                        description: description.clone(),
                        audio_url: media
                            .content
                            .first()
                            .unwrap()
                            .url
                            .as_ref()
                            .unwrap()
                            .to_string(),
                    })
            })
            .collect();
        eps.sort_by_key(|ep| ep.published_at);
        Ok(eps)
    }
}
