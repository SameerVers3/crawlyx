use std::sync::{Arc, Mutex};
use std::collections::{HashMap, HashSet};
use std::sync::RwLock;

pub mod utils;
use utils::new_id;



#[derive(Debug)]
pub struct NodeData {
    pub id: usize,
    pub url: String,
    pub data: Option<String>,
    pub parents: Vec<Arc<Mutex<NodeData>>>,
    pub children: Vec<Arc<Mutex<NodeData>>>,
    parent_ids: HashSet<usize>,
    child_ids: HashSet<usize>,
    pub depth: usize,
}

#[derive(Debug)]
pub struct Graph {
    nodes: RwLock<HashMap<String, Arc<Mutex<NodeData>>>>,
}

pub type Node = Arc<Mutex<NodeData>>;

impl NodeData {
    pub fn new(url: String, data: Option<String>, depth: usize) -> Node {
        Arc::new(Mutex::new(Self {
            id: new_id(),
            url,
            data,
            parents: Vec::new(),
            children: Vec::new(),
            parent_ids: HashSet::new(),
            child_ids: HashSet::new(),
            depth,
        }))
    }

    pub fn insert_parent(&mut self, node: Node) {
        let node_id = node.lock().unwrap().id;
        
        if self.parent_ids.insert(node_id) {
            self.parents.push(node);
        }
    }

    pub fn insert_child(&mut self, node: Node) {
        let node_id = node.lock().unwrap().id;

        if self.child_ids.insert(node_id) {
            self.children.push(node);
        }
    }
}

impl Graph {
    pub fn new(root_url: String) -> Self {
        let root_node = NodeData::new(root_url.clone(), None, 0);
        let mut map = HashMap::new();
        map.insert(root_url, Arc::clone(&root_node));

        Self {
            nodes: RwLock::new(map),
        }
    }

    pub fn add_node(&self, url: String, data: Option<String>, depth: usize) -> Node {
        let mut map = self.nodes.write().unwrap();

        if let Some(existing) = map.get(&url) { 
            return Arc::clone(existing);
        }

        let node = NodeData::new(url.clone(), data, depth);
        map.insert(url, Arc::clone(&node));
        node
    }

    pub fn get_node(&self, url: &str) -> Option<Node> {
        let map = self.nodes.read().unwrap();
        map.get(url).map(Arc::clone)
    }

    pub fn add_edge(&self, parent: &Node, child: &Node) {
        let parent_id = parent.lock().unwrap().id;
        let child_id  = child.lock().unwrap().id;

        // Always lock lower ID first — this is the deadlock prevention rule.
        if parent_id < child_id {
            parent.lock().unwrap().insert_child(Arc::clone(child));
            child.lock().unwrap().insert_parent(Arc::clone(parent));
        } else {
            child.lock().unwrap().insert_parent(Arc::clone(parent));
            parent.lock().unwrap().insert_child(Arc::clone(child));
        }
    }

    pub fn set_content(&self, node: &Node, content: String) {
        node.lock().unwrap().data = Some(content);
    }

    pub fn size(&self) -> usize {
        self.nodes.read().unwrap().len()
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn node_new_sets_fields() {
        let node = NodeData::new("https://example.com".to_string(), Some("hello".to_string()), 2);
        let inner = node.lock().unwrap();
        assert_eq!(inner.url, "https://example.com");
        assert_eq!(inner.data, Some("hello".to_string()));
        assert_eq!(inner.depth, 2);
        assert!(inner.parents.is_empty());
        assert!(inner.children.is_empty());
    }

    #[test]
    fn node_ids_are_unique() {
        let a = NodeData::new("https://a.com".to_string(), None, 0);
        let b = NodeData::new("https://b.com".to_string(), None, 0);
        let id_a = a.lock().unwrap().id;
        let id_b = b.lock().unwrap().id;
        assert_ne!(id_a, id_b);
    }


    #[test]
    fn graph_new_has_root() {
        let g = Graph::new("https://root.com".to_string());
        let root = g.get_node("https://root.com").unwrap();
        assert_eq!(root.lock().unwrap().url, "https://root.com");
        assert_eq!(root.lock().unwrap().depth, 0);
    }

    #[test]
    fn graph_new_root_is_in_node_map() {
        let g = Graph::new("https://root.com".to_string());
        let found = g.get_node("https://root.com");
        assert!(found.is_some());
    }

    #[test]
    fn add_node_inserts_new_node() {
        let g = Graph::new("https://root.com".to_string());
        g.add_node("https://page.com".to_string(), None, 1);
        assert!(g.get_node("https://page.com").is_some());
    }

    #[test]
    fn add_node_returns_same_arc_for_duplicate_url() {
        let g = Graph::new("https://root.com".to_string());
        let first  = g.add_node("https://page.com".to_string(), None, 1);
        let second = g.add_node("https://page.com".to_string(), None, 1);
        // Both Arcs must point to the same allocation
        assert!(Arc::ptr_eq(&first, &second));
    }

    #[test]
    fn add_node_does_not_overwrite_existing_data() {
        let g = Graph::new("https://root.com".to_string());
        g.add_node("https://page.com".to_string(), Some("original".to_string()), 1);
        g.add_node("https://page.com".to_string(), Some("overwrite attempt".to_string()), 1);
        let node = g.get_node("https://page.com").unwrap();
        assert_eq!(node.lock().unwrap().data, Some("original".to_string()));
    }

    #[test]
    fn get_node_returns_none_for_missing_url() {
        let g = Graph::new("https://root.com".to_string());
        assert!(g.get_node("https://not-here.com").is_none());
    }

    #[test]
    fn set_content_updates_data() {
        let g = Graph::new("https://root.com".to_string());
        let node = g.add_node("https://page.com".to_string(), None, 1);
        assert_eq!(node.lock().unwrap().data, None);
        g.set_content(&node, "<html>...</html>".to_string());
        assert_eq!(node.lock().unwrap().data, Some("<html>...</html>".to_string()));
    }


    #[test]
    fn add_edge_no_duplicate_edges() {
        let g = Graph::new("https://root.com".to_string());
        let parent = g.get_node("https://root.com").unwrap();
        let child  = g.add_node("https://child.com".to_string(), None, 1);

        g.add_edge(&parent, &child);
        g.add_edge(&parent, &child); 

        assert_eq!(parent.lock().unwrap().children.len(), 1);
        assert_eq!(child.lock().unwrap().parents.len(), 1);
    }    

    #[test]
    fn add_edge_supports_cycle() {
        let g = Graph::new("https://a.com".to_string());
        let a = g.get_node("https://a.com").unwrap();
        let b = g.add_node("https://b.com".to_string(), None, 1);

        g.add_edge(&a, &b);
        g.add_edge(&b, &a); 

        assert_eq!(a.lock().unwrap().children.len(), 1);
        assert_eq!(b.lock().unwrap().children.len(), 1); 
    }

    #[test]
    fn add_edge_links_parent_and_child() {

        let g = Graph::new("https://root.com".to_string());
        let parent = g.get_node("https://root.com").unwrap();
        let child  = g.add_node("https://child.com".to_string(), None, 1);
        g.add_edge(&parent, &child);

        {
            let p = parent.lock().unwrap();
            assert_eq!(p.children.len(), 1);
            let child_url = p.children[0].lock().unwrap().url.clone();
            assert_eq!(child_url, "https://child.com");
        }  


        {
            let c = child.lock().unwrap();
            assert_eq!(c.parents.len(), 1);
            let parent_url = c.parents[0].lock().unwrap().url.clone();
            assert_eq!(parent_url, "https://root.com");
        }
    }

    #[test]
    fn concurrent_add_edge_does_not_deadlock() {
        let g = Arc::new(Graph::new("https://root.com".to_string()));
        let a = g.add_node("https://a.com".to_string(), None, 1);
        let b = g.add_node("https://b.com".to_string(), None, 1);

        let g1 = Arc::clone(&g);
        let a1 = Arc::clone(&a);
        let b1 = Arc::clone(&b);

        let g2 = Arc::clone(&g);
        let a2 = Arc::clone(&a);
        let b2 = Arc::clone(&b);

        // T1: add A to B
        let t1 = thread::spawn(move || {
            g1.add_edge(&a1, &b1);
        });

        // T3: add B to A (opposite direction => deadlock hehe)
        let t2 = thread::spawn(move || {
            g2.add_edge(&b2, &a2);
        });

        t1.join().unwrap();
        t2.join().unwrap();
    }
}    
