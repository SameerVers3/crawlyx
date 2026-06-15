use std::collections::HashSet;
use crate::graph::Graph;
use serde::Serialize;

#[derive(Serialize, Clone, Debug)]
pub struct TreeNode {
    pub url: String,
    pub children: Vec<TreeNode>,
}

pub fn derive_tree(graph: &Graph, root_url: &str) -> Option<TreeNode> {
    let mut visited = HashSet::new();
    build_tree_node(graph, root_url, &mut visited)
}

fn build_tree_node(graph: &Graph, url: &str, visited: &mut HashSet<String>) -> Option<TreeNode> {
    if visited.contains(url) {
        return None;
    }
    visited.insert(url.to_string());

    let node = graph.get_node(url)?;
    let node_lock = node.lock().unwrap();
    
    let mut children_nodes = Vec::new();
    for child in &node_lock.children {
        let child_url = child.lock().unwrap().url.clone();
        if let Some(child_tree) = build_tree_node(graph, &child_url, visited) {
            children_nodes.push(child_tree);
        }
    }

    Some(TreeNode {
        url: url.to_string(),
        children: children_nodes,
    })
}
