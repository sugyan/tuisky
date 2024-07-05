use super::config::Config;
use super::types::{SavedFeed, SavedFeedValue};
use bsky_sdk::api::app::bsky::actor::defs::SavedFeedData;
use bsky_sdk::api::app::bsky::feed::defs::{FeedViewPost, FeedViewPostReasonRefs};
use bsky_sdk::api::types::string::Cid;
use bsky_sdk::api::types::{Object, Union};
use bsky_sdk::preference::Preferences;
use bsky_sdk::BskyAgent;
use bsky_sdk::Result;
use indexmap::IndexMap;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch::{self, Receiver, Sender};
use tokio::task::JoinHandle;
use tokio::time;

pub struct Watcher {
    pub agent: Arc<BskyAgent>,
    config: Config,
    preferences: Sender<Preferences>,
    handles: Vec<JoinHandle<()>>,
}

impl Watcher {
    pub fn new(agent: Arc<BskyAgent>, config: Config) -> Self {
        let (preferences, mut rx) = watch::channel(Preferences::default());
        let mut handles = Vec::new();
        {
            handles.push(tokio::spawn(async move {
                while rx.changed().await.is_ok() {
                    let _ = rx.borrow_and_update();
                    log::info!("Preferences updated");
                }
            }));
            let (agent, tx) = (agent.clone(), preferences.clone());
            handles.push(tokio::spawn(async move {
                let mut preferences_interval =
                    time::interval(Duration::from_secs(config.intervals.preferences));
                loop {
                    let preferences_tick = preferences_interval.tick();
                    tokio::select! {
                        _ = preferences_tick => {
                            if let Ok(prefs) = agent.get_preferences(true).await {
                                if let Err(e) = tx.send(prefs.clone()) {
                                    log::warn!("failed to send preferences data: {e}");
                                }
                            } else {
                                log::warn!("failed to get preferences");
                            }
                        }
                    }
                }
            }));
        }
        Self {
            agent,
            config,
            preferences,
            handles,
        }
    }
    pub fn stop(&self) {
        for handle in &self.handles {
            handle.abort();
        }
    }
    pub fn saved_feeds(&self, init: Vec<SavedFeed>) -> Receiver<Vec<SavedFeed>> {
        let (tx, rx) = watch::channel(init);
        let agent = self.agent.clone();
        let mut preferences = self.preferences.subscribe();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    changed = preferences.changed() => {
                        if changed.is_err()  {
                            return log::warn!("preferences channel closed");
                        }
                        let saved_feeds = preferences.borrow_and_update().saved_feeds.clone();
                        match collect_feeds(&agent, &saved_feeds).await {
                            Ok(feeds) => {
                                if let Err(e) = tx.send(feeds) {
                                    log::warn!("failed to send saved feeds: {e}");
                                }
                            }
                            Err(e) => {
                                log::warn!("failed to collect feeds {e:?}");
                            }
                        }
                    }
                    _ = tx.closed() => {
                        return log::warn!("saved feeds channel closed");
                    }
                }
            }
        });
        rx
    }
    pub fn feed_views(
        &self,
        init: IndexMap<Cid, FeedViewPost>,
        feed: &SavedFeedValue,
    ) -> Receiver<IndexMap<Cid, FeedViewPost>> {
        let (tx, rx) = watch::channel(init.clone());
        let agent = self.agent.clone();
        let feed = feed.clone();
        let interval = self.config.intervals.feed_view_posts;
        let mut feed_map = init;
        tokio::spawn(async move {
            let mut interval = time::interval(Duration::from_secs(interval));
            loop {
                let tick = interval.tick();
                tokio::select! {
                    _ = tick => {
                        match fetch_feed_views(&agent, &feed, &mut feed_map).await {
                            Ok(()) => {
                                if let Err(e) = tx.send(feed_map.clone()) {
                                    log::warn!("failed to send feed views: {e}");
                                }
                            }
                            Err(e) => {
                                log::warn!("failed to fetch feed views {e:?}");
                            }
                        }
                    }
                    _ = tx.closed() => {
                        return log::warn!("feed views channel closed");
                    }
                }
            }
        });
        rx
    }
}

async fn collect_feeds(
    agent: &Arc<BskyAgent>,
    saved_feeds: &[Object<SavedFeedData>],
) -> Result<Vec<SavedFeed>> {
    let feeds = saved_feeds
        .iter()
        .filter_map(|feed| {
            if feed.r#type == "feed" {
                Some(feed.value.clone())
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    let feed_generators = agent
        .api
        .app
        .bsky
        .feed
        .get_feed_generators(
            bsky_sdk::api::app::bsky::feed::get_feed_generators::ParametersData { feeds }.into(),
        )
        .await
        .map(|output| {
            output
                .feeds
                .iter()
                .map(|feed| (feed.uri.clone(), feed.clone()))
                .collect::<HashMap<_, _>>()
        })?;
    // TODO: list
    let mut feeds = Vec::new();
    for data in saved_feeds {
        match data.r#type.as_str() {
            "feed" => {
                if let Some(feed) = feed_generators.get(&data.value) {
                    feeds.push(SavedFeed {
                        pinned: data.pinned,
                        value: SavedFeedValue::Feed(Box::new(feed.clone())),
                    });
                }
            }
            "list" => {
                // TODO
            }
            "timeline" => {
                feeds.push(SavedFeed {
                    pinned: data.pinned,
                    value: SavedFeedValue::Timeline(data.value.clone()),
                });
            }
            _ => {}
        }
    }
    Ok(feeds)
}

async fn fetch_feed_views(
    agent: &Arc<BskyAgent>,
    feed: &SavedFeedValue,
    feed_map: &mut IndexMap<Cid, FeedViewPost>,
) -> Result<()> {
    let mut feed_views = match feed {
        SavedFeedValue::Feed(generator_view) => {
            agent
                .api
                .app
                .bsky
                .feed
                .get_feed(
                    bsky_sdk::api::app::bsky::feed::get_feed::ParametersData {
                        cursor: None,
                        feed: generator_view.uri.clone(),
                        limit: 30.try_into().ok(),
                    }
                    .into(),
                )
                .await?
                .data
                .feed
        }
        SavedFeedValue::List => Vec::new(),
        SavedFeedValue::Timeline(_) => {
            agent
                .api
                .app
                .bsky
                .feed
                .get_timeline(
                    bsky_sdk::api::app::bsky::feed::get_timeline::ParametersData {
                        algorithm: None,
                        cursor: None,
                        limit: 30.try_into().ok(),
                    }
                    .into(),
                )
                .await?
                .data
                .feed
        }
    };
    feed_views.reverse();
    update_feeds(&feed_views, feed_map);
    Ok(())
}

fn update_feeds(feed_views: &[FeedViewPost], feed_map: &mut IndexMap<Cid, FeedViewPost>) {
    for feed_view in feed_views {
        if let Some(entry) = feed_map.get_mut(&feed_view.post.cid) {
            // Is the feed view a new repost?
            if match (&entry.reason, &feed_view.reason) {
                (
                    Some(Union::Refs(FeedViewPostReasonRefs::ReasonRepost(curr))),
                    Some(Union::Refs(FeedViewPostReasonRefs::ReasonRepost(next))),
                ) => curr.indexed_at < next.indexed_at,
                (None, Some(_)) => true,
                _ => false,
            } {
                // Remove the old entry
                feed_map.swap_remove(&feed_view.post.cid);
            } else {
                // Just update the post
                entry.post = feed_view.post.clone();
                continue;
            }
        }
        feed_map.insert(feed_view.post.cid.clone(), feed_view.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bsky_sdk::api::app::bsky::actor::defs::{ProfileViewBasic, ProfileViewBasicData};
    use bsky_sdk::api::app::bsky::feed::defs::{FeedViewPostData, PostViewData, ReasonRepostData};
    use bsky_sdk::api::records::Record;
    use bsky_sdk::api::types::{string::Datetime, UnknownData};
    use ipld_core::ipld::Ipld;
    use std::collections::BTreeMap;

    fn feed_view_post(cid: Cid, reason_indexed_at: Option<Datetime>) -> FeedViewPost {
        fn profile_view_basic() -> ProfileViewBasic {
            ProfileViewBasicData {
                associated: None,
                avatar: None,
                created_at: None,
                did: "did:fake:post.test".parse().expect("invalid did"),
                display_name: None,
                handle: "post.test".parse().expect("invalid handle"),
                labels: None,
                viewer: None,
            }
            .into()
        }

        FeedViewPostData {
            feed_context: None,
            post: PostViewData {
                author: profile_view_basic(),
                cid,
                embed: None,
                indexed_at: Datetime::now(),
                labels: None,
                like_count: None,
                record: Record::Unknown(UnknownData {
                    r#type: "post".to_string(),
                    data: Ipld::Map(BTreeMap::new()),
                }),
                reply_count: None,
                repost_count: None,
                threadgate: None,
                uri: String::new(),
                viewer: None,
            }
            .into(),
            reason: reason_indexed_at.map(|indexed_at| {
                Union::Refs(FeedViewPostReasonRefs::ReasonRepost(Box::new(
                    ReasonRepostData {
                        by: profile_view_basic(),
                        indexed_at,
                    }
                    .into(),
                )))
            }),
            reply: None,
        }
        .into()
    }

    #[test]
    fn update_feed_views() {
        let cids = [
            "bafyreidfayvfuwqa7qlnopdjiqrxzs6blmoeu4rujcjtnci5beludirz2a"
                .parse::<Cid>()
                .expect("invalid cid"),
            "bafyreidfayvfuwqa7qlnopdjiqrxzs6blmoeu4rujcjtnci5beludirz3a"
                .parse::<Cid>()
                .expect("invalid cid"),
            "bafyreidfayvfuwqa7qlnopdjiqrxzs6blmoeu4rujcjtnci5beludirz4a"
                .parse::<Cid>()
                .expect("invalid cid"),
        ];
        let mut feed_map = IndexMap::new();
        // Empty feeds
        update_feeds(&Vec::new(), &mut feed_map);
        assert_eq!(feed_map.len(), 0);
        // New feed
        update_feeds(&[feed_view_post(cids[0].clone(), None)], &mut feed_map);
        assert_eq!(feed_map.len(), 1);
        // Duplicate feed
        update_feeds(&[feed_view_post(cids[0].clone(), None)], &mut feed_map);
        assert_eq!(feed_map.len(), 1);
        // Duplicated and new feed
        update_feeds(
            &[
                feed_view_post(cids[0].clone(), None),
                feed_view_post(cids[1].clone(), None),
            ],
            &mut feed_map,
        );
        assert_eq!(feed_map.len(), 2);
        assert_eq!(feed_map[0].post.cid, cids[0]);
        assert_eq!(feed_map[1].post.cid, cids[1]);
        // New and duplicated feed
        update_feeds(
            &[
                feed_view_post(cids[2].clone(), None),
                feed_view_post(cids[1].clone(), None),
            ],
            &mut feed_map,
        );
        assert_eq!(feed_map.len(), 3);
        assert_eq!(feed_map[0].post.cid, cids[0]);
        assert_eq!(feed_map[1].post.cid, cids[1]);
        assert_eq!(feed_map[2].post.cid, cids[2]);
        // Duplicated, but updated feed
        update_feeds(
            &[
                feed_view_post(cids[1].clone(), Some(Datetime::now())),
                feed_view_post(cids[2].clone(), None),
            ],
            &mut feed_map,
        );
        assert_eq!(feed_map.len(), 3);
        println!("{:?}", feed_map.keys().collect::<Vec<_>>());
        assert_eq!(feed_map[0].post.cid, cids[0]);
        assert_eq!(feed_map[1].post.cid, cids[2]);
        assert_eq!(feed_map[2].post.cid, cids[1]);
        assert!(feed_map[0].reason.is_none());
        assert!(feed_map[1].reason.is_none());
        assert!(feed_map[2].reason.is_some());
        // Duplicated, but updated feed
        update_feeds(
            &[
                feed_view_post(cids[0].clone(), Some(Datetime::now())),
                feed_view_post(cids[1].clone(), Some(Datetime::now())),
            ],
            &mut feed_map,
        );
        assert_eq!(feed_map.len(), 3);
        println!("{:?}", feed_map.keys().collect::<Vec<_>>());
        assert_eq!(feed_map[1].post.cid, cids[2]);
        assert_eq!(feed_map[0].post.cid, cids[0]);
        assert_eq!(feed_map[2].post.cid, cids[1]);
        assert!(feed_map[0].reason.is_some());
        assert!(feed_map[1].reason.is_none());
        assert!(feed_map[2].reason.is_some());
    }
}
