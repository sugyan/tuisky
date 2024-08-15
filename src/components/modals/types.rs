use bsky_sdk::api::com::atproto::repo::strong_ref;

#[derive(Clone, Default, PartialEq, Eq)]
pub struct EmbedData {
    pub images: Vec<ImageData>,
    pub record: Option<strong_ref::Main>,
}

#[derive(Clone, PartialEq, Eq)]
pub struct ImageData {
    pub path: String,
    pub alt: String,
}

#[derive(Clone)]
pub enum Data {
    Embed(EmbedData),
    Image((ImageData, Option<usize>)),
    Record(strong_ref::Main),
}

#[derive(Clone)]
pub enum Action {
    Ok(Data),
    Delete(Option<usize>),
    Cancel,
    Render,
}
