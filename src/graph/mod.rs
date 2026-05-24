pub mod utils;
use utils::{new_id};

#[derive(Debug)]
pub struct NodeData {
    id: usize,
    url: String
    data: Option<String>,
    parent: Vec<NodeData>,
    children: Vec<NodeData>,
    depth: usize
}

#[derive(Debug)]
pub struct Graph {
    root: NodeData
}


impl NodeData {
    pub fn new(url: String, data: Option<String>, depth: usize) -> Self {
        Self {



            
            id: new_id(),
            url,
            data,
            parent: new Vec<NodeData>,
            children: new Vec<NodeData>,
            depth: usize
        }
    }

    pub fn insert_parent(&mut self, node: NodeData) -> {
        // check if already exist;
        
        if !self.parent.iter().any(|&i| i.id == node.id) {
            self.parent.push(node);
        }


    }

    pub fn insert_children(&mut self, node: NodeData) -> {
        if !self.parent.iter().any(|&i| i.id == node.id) {
            self.children.push(node);
        }

    }




}

impl Graph {

    pub fn new(root: NodeData) {

    }



}

