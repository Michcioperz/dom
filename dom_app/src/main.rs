use cursive::{event::EventResult, traits::Scrollable, Cursive};
use dashmap::DashMap;
use dom_api::*;
use rayon::prelude::{IntoParallelIterator, ParallelIterator};

const XDG_PREFIX: &str = "dom314";

fn is_gui() -> bool {
    std::env::var_os("DISPLAY").is_some()
}

fn listen(db: &Db, url: &str) -> anyhow::Result<()> {
    use std::process::Stdio;
    let mut cmd = std::process::Command::new("mpv");
    let cmd = cmd.arg(url);
    if is_gui() {
        cmd.arg("--force-window")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
    } else {
        cmd
    }
    .spawn()?;
    db.mark_listened(url, true)?;
    Ok(())
}

#[derive(Debug, Clone)]
struct Db {
    inner: sled::Db,
}

impl Db {
    fn open() -> anyhow::Result<Db> {
        let path =
            xdg::BaseDirectories::with_prefix(XDG_PREFIX)?.place_data_file("database.sled")?;
        let inner = sled::open(path)?;
        Ok(Db { inner })
    }

    fn was_listened(&self, url: &str) -> anyhow::Result<bool> {
        Ok(self.inner.open_tree("listened")?.contains_key(url)?)
    }

    fn mark_listened(&self, url: &str, value: bool) -> anyhow::Result<()> {
        self.inner
            .open_tree("listened")?
            .fetch_and_update(url, |_| if value { Some("") } else { None })?;
        Ok(())
    }

    fn is_in_group(&self, group: &'static str, url: &str) -> anyhow::Result<bool> {
        Ok(self.inner.open_tree(group)?.contains_key(url)?)
    }

    fn change_subscription(
        &self,
        group: &'static str,
        url: &str,
        backend: &'static str,
        new_status: bool,
    ) -> anyhow::Result<()> {
        self.inner.open_tree(group)?.fetch_and_update(url, |_| {
            if new_status {
                Some(backend)
            } else {
                None
            }
        })?;
        Ok(())
    }

    fn list_subscriptions(&self, group: &'static str) -> anyhow::Result<Vec<(String, String)>> {
        let byte_pairs_res: Result<Vec<_>, _> = self.inner.open_tree(group)?.iter().collect();
        let byte_pairs = byte_pairs_res?;
        let byte_pairs_iter = byte_pairs.into_iter();
        let str_pairs_iter = byte_pairs_iter.map(|(url_bytes, backend_bytes)| {
            (
                String::from_utf8_lossy(&*url_bytes).to_string(),
                String::from_utf8_lossy(&*backend_bytes).to_string(),
            )
        });
        Ok(str_pairs_iter.collect())
    }
}

lazy_static::lazy_static! {
    static ref FEED_CACHE: DashMap<String, Vec<Episode>> = DashMap::new();
}

fn fetch_feed(backend: &str, url: &str) -> Result<Vec<Episode>, anyhow::Error> {
    match FEED_CACHE.get(url) {
        Some(episodes) => Ok(episodes.clone()),
        None => Ok(FEED_CACHE
            .entry(url.to_string())
            .or_insert_with(|| get_backend(backend).fetch_feed(url).unwrap())
            .clone()),
    }
}

fn group_view(db: Db, group: &'static str) -> impl cursive::View {
    let feeds = db.list_subscriptions(group).unwrap();
    let feeds_contents: Result<Vec<_>, _> = feeds
        .into_par_iter()
        .map(|(url, backend)| fetch_feed(&backend, &url))
        .collect();
    let episodes = feeds_contents.unwrap().into_iter().flatten().collect();
    cursive::views::Dialog::around(episodes_subview(
        db.clone(),
        episodes,
        true,
        Box::new(move |siv| {
            siv.pop_layer();
            siv.add_layer(group_view(db.clone(), group));
        }),
    ))
    .title(group)
    .dismiss_button("Go back")
}

fn episodes_subview(
    db: Db,
    mut episodes: Vec<Episode>,
    only_unlistened: bool,
    refresh: Box<dyn Fn(&mut Cursive)>,
) -> impl cursive::View {
    episodes.sort_by(|a, b| a.published_at.cmp(&b.published_at).reverse());
    cursive::views::OnEventView::new(
        cursive::views::SelectView::new()
            .with_all(
                episodes
                    .into_iter()
                    .filter(|ep| !only_unlistened || !db.was_listened(&ep.audio_url).unwrap())
                    .map(|ep| {
                        (
                            format!(
                                "{}[{}] [{}] {}",
                                if only_unlistened {
                                    ""
                                } else if db.was_listened(&ep.audio_url).unwrap() {
                                    "    "
                                } else {
                                    "[*] "
                                },
                                ep.published_at.date().naive_local(),
                                &ep.podcast,
                                &ep.title,
                            ),
                            ep.audio_url,
                        )
                    }),
            )
            .on_submit({
                let db = db.clone();
                move |siv, url| {
                    listen(&db, url).unwrap();
                    refresh(siv);
                }
            }),
    )
    .on_event_inner('r', {
        let db = db.clone();
        move |view, _ev| {
            let db = db.clone();
            view.selection().map(|ep| {
                EventResult::with_cb(move |_siv| {
                    db.clone().mark_listened(&ep.clone(), true).unwrap();
                    // TODO: refresh
                })
            })
        }
    })
    .on_event_inner('R', {
        move |view, _ev| {
            let db = db.clone();
            view.selection().map(|ep| {
                EventResult::with_cb(move |_siv| {
                    db.clone().mark_listened(&ep.clone(), false).unwrap();
                    // TODO: refresh
                })
            })
        }
    })
    .scrollable()
}

fn podcast_view(db: &Db, podcast: &Podcast) -> anyhow::Result<impl cursive::View> {
    let intro = cursive::views::TextView::new(podcast.description.clone());
    let episodes_list = fetch_feed(podcast.backend, &podcast.feed_url)?;
    let mut dialog = cursive::views::Dialog::around(
        cursive::views::ListView::new()
            .child("Metadata", intro)
            .child(
                "Episodes",
                episodes_subview(db.clone(), episodes_list, false, {
                    let db = db.clone();
                    let podcast = podcast.clone();
                    Box::new(move |siv| {
                        siv.pop_layer();
                        siv.add_layer(podcast_view(&db, &podcast.clone()).unwrap());
                    })
                }),
            ),
    )
    .title(podcast.title.clone());
    for group in &["beloved", "timekilling"] {
        let podcast = podcast.clone();
        let db = db.clone();
        let subscribed = db.is_in_group(group, &podcast.feed_url)?;
        let action_char = if subscribed { "-" } else { "+" };
        dialog.add_button(format!("{}{}", action_char, group), move |siv| {
            db.change_subscription(group, &podcast.feed_url, podcast.backend, !subscribed)
                .unwrap();
            siv.pop_layer();
            siv.add_layer(podcast_view(&db, &podcast.clone()).unwrap());
        });
    }
    Ok(dialog.dismiss_button("Go back"))
}

fn get_backend(name: &str) -> &'static dyn FetchingBackend {
    match name {
        "rss" => dom_rss::FETCHING_BACKEND,
        #[cfg(feature = "radio357")]
        "radio357" => dom_radio357::FETCHING_BACKEND,
        _ => panic!("unknown backend: {}", name),
    }
}

fn discovery_selector(db: Db) -> impl cursive::View {
    let mut view = cursive::views::SelectView::<&'static dyn DiscoveryBackend>::new().autojump();
    view = view.item("Michcio's picks", dom_michcio::DISCOVERY_BACKEND);
    #[cfg(feature = "radio357")]
    {
        view = view.item("Radio 357", dom_radio357::DISCOVERY_BACKEND);
    }
    view = view.on_submit(move |siv, backend: &&'static dyn DiscoveryBackend| {
        let db = db.clone();
        let backend_list = cursive::views::SelectView::new()
            .autojump()
            .with_all(
                backend
                    .discovery()
                    .unwrap()
                    .into_iter()
                    .map(|pod| (pod.title.clone(), pod)),
            )
            .on_submit(move |siv, pod| {
                siv.add_layer(podcast_view(&db, &pod.clone()).unwrap());
            })
            .scrollable();
        let backend_view = cursive::views::Dialog::around(backend_list)
            .title("New podcasts")
            .dismiss_button("Go back");
        siv.add_layer(backend_view);
    });
    cursive::views::Dialog::around(view)
        .title("Discovery backends")
        .dismiss_button("Go back")
}

fn main_menu(db: Db) -> impl cursive::View {
    enum Action {
        Beloved,
        Timekilling,
        Undiscovered,
    }
    cursive::views::Dialog::around(
        cursive::views::SelectView::new()
            .autojump()
            .item("Beloved", Action::Beloved)
            .item("Timekilling", Action::Timekilling)
            .item("Undiscovered", Action::Undiscovered)
            .on_submit(move |siv, action| match *action {
                Action::Beloved => siv.add_layer(group_view(db.clone(), "beloved")),
                Action::Timekilling => siv.add_layer(group_view(db.clone(), "timekilling")),
                Action::Undiscovered => siv.add_layer(discovery_selector(db.clone())),
            }),
    )
    .title("Main menu")
    .button("Wipe cache", |_| FEED_CACHE.clear())
    .button("Exit", Cursive::quit)
}

fn main() -> anyhow::Result<()> {
    let db = Db::open()?;
    let mut siv = cursive::default();
    match xdg::BaseDirectories::with_prefix(XDG_PREFIX)?.find_config_file("theme.toml") {
        None => {}
        Some(theme_path) => siv.load_theme_file(theme_path).unwrap(),
    }
    siv.add_layer(main_menu(db));
    siv.run();
    Ok(())
}
