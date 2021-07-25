use dom_api::*;

pub const DISCOVERY_BACKEND: &dyn DiscoveryBackend = &MichciosPicks {};

struct MichciosPicks {}

impl DiscoveryBackend for MichciosPicks {
    fn discovery(&self) -> Result<Vec<Podcast>, anyhow::Error> {
        Ok(vec![
           Podcast {
               backend: "rss",
               feed_url: "https://2pady.pl/feed/podcast".to_string(),
               title: "2pady.pl".to_string(),
               description: "Między indie a mainstreamem, casualem a hardcorem, rozrywką a tworzeniem - o grach, z wyobraźnią. Współtworzony przez pasjonatów na co dzień pracujących w branży, pełen pierwszych wrażeń, recenzji i relacji. Już ponad 10 lat snujemy opowieści o krnąbrnych herosach, zawiłych intrygach, poruszających narracjach, całych krainach najeżonych potworami i magicznymi artefaktami. A o czym opowiemy dzisiaj?".to_string(),
           },
        ])
    }
}
