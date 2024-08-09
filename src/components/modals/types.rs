#[derive(Clone, Default, PartialEq, Eq)]
pub struct EmbedData {
    pub images: Vec<ImageData>,
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
}

#[derive(Clone)]
pub enum Action {
    Ok(Data),
    Delete(Option<usize>),
    Cancel,
    Render,
}
