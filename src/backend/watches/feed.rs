use super::super::types::SavedFeedValue;
use super::super::{Watch, Watcher};
use bsky_sdk::api::app::bsky::feed::defs::{FeedViewPost, FeedViewPostReasonRefs};
use bsky_sdk::api::types::string::Cid;
use bsky_sdk::api::types::Union;
use bsky_sdk::Result;
use bsky_sdk::{preference::Preferences, BskyAgent};
use indexmap::IndexMap;
use std::sync::Arc;
use tokio::sync::{broadcast, watch};

impl Watcher {
    pub fn feed(&self, feed_value: SavedFeedValue) -> impl Watch<Output = Vec<FeedViewPost>> {
        let (tx, _) = broadcast::channel(1);
        FeedWatcher {
            feed_value,
            agent: self.agent.clone(),
            preferences: self.preferences(),
            tx,
        }
    }
}

pub struct FeedWatcher<W> {
    feed_value: SavedFeedValue,
    agent: Arc<BskyAgent>,
    preferences: W,
    tx: broadcast::Sender<()>,
}

impl<W> Watch for FeedWatcher<W>
where
    W: Watch<Output = Preferences>,
{
    type Output = Vec<FeedViewPost>;

    fn subscribe(&self) -> tokio::sync::watch::Receiver<Self::Output> {
        let (tx, rx) = watch::channel(Default::default());
        let (agent, feed_value) = (self.agent.clone(), Arc::new(self.feed_value.clone()));
        let (mut preferences, mut quit) = (self.preferences.subscribe(), self.tx.subscribe());
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    changed = preferences.changed() => {
                        if changed.is_ok() {
                            let preferences = preferences.borrow_and_update().clone();
                            let (agent, feed_value, tx) = (agent.clone(), feed_value.clone(), tx.clone());
                            tokio::spawn(async move {
                                update(&agent, &feed_value, &preferences, &tx).await;
                            });
                        } else {
                            break log::warn!("preferences channel closed");
                        }
                    }
                    _ = quit.recv() => {
                        break;
                    }
                }
            }
            log::debug!("quit");
        });
        rx
    }
    fn unsubscribe(&self) {
        if let Err(e) = self.tx.send(()) {
            log::error!("failed to send quit: {e}");
        }
        self.preferences.unsubscribe();
    }
    fn refresh(&self) {
        self.preferences.refresh();
    }
}

async fn update(
    agent: &BskyAgent,
    feed_value: &SavedFeedValue,
    preferences: &Preferences,
    tx: &watch::Sender<Vec<FeedViewPost>>,
) {
    // TODO: moderation, resume from current state
    match get_feed_view_posts(agent, feed_value).await {
        Ok(feed_view_posts) => {
            tx.send(feed_view_posts).ok();
        }
        Err(e) => {
            log::error!("failed to get feed view posts: {e}");
        }
    }
}

async fn get_feed_view_posts(
    agent: &BskyAgent,
    feed_value: &SavedFeedValue,
) -> Result<Vec<FeedViewPost>> {
    let mut feed = match feed_value {
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
    log::debug!("fetch {} feed view posts", feed.len());
    feed.reverse();
    let mut feed_map = IndexMap::new();
    update_feeds(&feed, &mut feed_map);
    Ok(feed_map.values().rev().cloned().collect())
}

fn update_feeds(feed: &[FeedViewPost], feed_map: &mut IndexMap<Cid, FeedViewPost>) {
    for post in feed {
        if let Some(entry) = feed_map.get_mut(&post.post.cid) {
            // Is the feed view a new repost?
            if match (&entry.reason, &post.reason) {
                (
                    Some(Union::Refs(FeedViewPostReasonRefs::ReasonRepost(curr))),
                    Some(Union::Refs(FeedViewPostReasonRefs::ReasonRepost(next))),
                ) => curr.indexed_at < next.indexed_at,
                (None, Some(_)) => true,
                _ => false,
            } {
                // Remove the old entry
                feed_map.swap_remove(&post.post.cid);
            } else {
                // Just update the post
                entry.post = post.post.clone();
                continue;
            }
        }
        feed_map.insert(post.post.cid.clone(), post.clone());
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
