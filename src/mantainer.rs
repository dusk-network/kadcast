use crate::{kbucket::Tree, peer::PeerInfo};

pub(crate) struct TableMantainer {
    bootstrapping_nodes: Vec<String>,
    ktable: Tree<PeerInfo>,
}

impl TableMantainer {
    pub fn new(bootstrapping_nodes: Vec<String>, ktable: Tree<PeerInfo>) -> Self {
        TableMantainer {
            bootstrapping_nodes,
            ktable,
        }
    }

    pub fn bootstrap(&mut self) {}

    pub fn ktable(&self) -> &Tree<PeerInfo> {
        &self.ktable
    }
}
