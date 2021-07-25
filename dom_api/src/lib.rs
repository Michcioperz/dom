#[derive(Debug, Clone)]
pub struct Podcast {
    pub backend: &'static str,
    pub feed_url: String,
    pub title: String,
    pub description: String,
}

#[derive(Debug, Clone)]
pub struct Episode {
    pub podcast: String,
    pub title: String,
    pub description: String,
    pub published_at: chrono::DateTime<chrono::Local>,
    pub audio_url: String,
}

pub trait FetchingBackend {
    fn fetch_feed(&self, url: &str) -> Result<Vec<Episode>, anyhow::Error>;
}

pub trait DiscoveryBackend {
    fn discovery(&self) -> Result<Vec<Podcast>, anyhow::Error>;

    fn search(&self, query: &str) -> Result<Vec<Podcast>, anyhow::Error> {
        self.discovery().map(|vec| {
            vec.into_iter()
                .filter(|pod| pod.title.contains(query) || pod.description.contains(query))
                .collect()
        })
    }
}
