use serde::Deserialize;

#[derive(Deserialize, Clone, Debug)]
pub struct Asset {
    pub name: String,
    pub url: String,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Release {
    pub tag_name: String,
    pub assets: Vec<Asset>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Tag {
    pub name: String,
    pub commit: Commit,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Commit {
    pub sha: String,
}

#[derive(Clone, Debug)]
pub struct ReleaseAndTag {
    pub release: Release,
    pub tag: Tag,
}
