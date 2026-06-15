use url::Url;

pub type NodeId = String;

#[derive(Clone, Debug)]
pub struct WorkUnit {
    pub url: Url,
    pub depth: usize,
    pub parent_node_id: Option<NodeId>,
}
