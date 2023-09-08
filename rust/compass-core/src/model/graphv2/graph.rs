use compass_core::model::graphv2::edge_loader::EdgeLoaderConfig;
use compass_core::model::graphv2::graph_config::GraphConfig;
use compass_core::model::graphv2::graph_error::GraphError;
use compass_core::model::graphv2::vertex_loader::VertexLoaderConfig;

use crate::algorithm::search::min_search_tree::direction::Direction;
use crate::model::property::edge::Edge;
use crate::model::property::vertex::Vertex;

use super::edge_id::EdgeId;
use super::graph_error::GraphError;
use super::vertex_id::VertexId;

pub struct Graph {
    pub adj: Vec<HashMap<EdgeId, VertexId>>,
    pub rev: Vec<HashMap<EdgeId, VertexId>>,
    pub edges: Vec<Edge>,
    pub vertices: Vec<Vertex>,
}

impl Graph {
    fn n_edges(&self) -> usize {
        self.edges.len()
    }
    fn n_vertices(&self) -> usize {
        self.vertices.len()
    }
    fn all_edge_ids(&self) -> Vec<EdgeId> {
        self.edges.iter().map(|edge| edge.edge_id).collect()
    }
    fn all_edges(&self) -> Vec<Edge> {
        self.edges.iter().cloned().collect()
    }
    fn all_vertex_ids(&self) -> Vec<VertexId> {
        self.vertices
            .iter()
            .map(|vertex| vertex.vertex_id)
            .collect()
    }
    fn all_vertices(&self) -> Vec<Vertex> {
        self.vertices.iter().cloned().collect()
    }
    fn edge_attr(&self, edge_id: EdgeId) -> Result<&Edge, GraphError> {
        match self.edges.get(edge_id.0 as usize) {
            None => Err(GraphError::EdgeAttributeNotFound { edge_id }),
            Some(edge) => Ok(edge),
        }
    }
    fn vertex_attr(&self, vertex_id: VertexId) -> Result<&Vertex, GraphError> {
        match self.vertices.get(vertex_id.0 as usize) {
            None => Err(GraphError::VertexAttributeNotFound { vertex_id }),
            Some(vertex) => Ok(vertex),
        }
    }
    fn out_edges(&self, src: VertexId) -> Result<Vec<EdgeId>, GraphError> {
        match self.adj.get(src.0 as usize) {
            None => Err(GraphError::VertexWithoutOutEdges { vertex_id: src }),
            Some(out_map) => {
                let edge_ids = out_map.keys().cloned().collect();
                Ok(edge_ids)
            }
        }
    }
    fn in_edges(&self, src: VertexId) -> Result<Vec<EdgeId>, GraphError> {
        match self.rev.get(src.0 as usize) {
            None => Err(GraphError::VertexWithoutInEdges { vertex_id: src }),
            Some(in_map) => {
                let edge_ids = in_map.keys().cloned().collect();
                Ok(edge_ids)
            }
        }
    }
    fn src_vertex(&self, edge_id: EdgeId) -> Result<VertexId, GraphError> {
        self.edge_attr(edge_id).map(|e| e.src_vertex_id)
    }
    fn dst_vertex(&self, edge_id: EdgeId) -> Result<VertexId, GraphError> {
        self.edge_attr(edge_id).map(|e| e.dst_vertex_id)
    }

    /// helper function to give incident edges to a vertex based on a
    /// traversal direction.
    fn incident_edges(
        &self,
        vertex_id: VertexId,
        direction: Direction,
    ) -> Result<Vec<EdgeId>, GraphError> {
        match direction {
            Direction::Forward => self.out_edges(vertex_id),
            Direction::Reverse => self.in_edges(vertex_id),
        }
    }

    /// helper function to give the incident vertex to an edge based on a
    /// traversal direction.
    fn incident_vertex(
        &self,
        edge_id: EdgeId,
        direction: Direction,
    ) -> Result<VertexId, GraphError> {
        match direction {
            Direction::Forward => self.dst_vertex(edge_id),
            Direction::Reverse => self.src_vertex(edge_id),
        }
    }

    fn edge_triplet_attrs(&self, edge_id: EdgeId) -> Result<(&Vertex, &Edge, &Vertex), GraphError> {
        let edge = self.edge_attr(edge_id)?;
        let src = self.vertex_attr(edge.src_vertex_id)?;
        let dst = self.vertex_attr(edge.dst_vertex_id)?;

        Ok((src, edge, dst))
    }

    /// helper function to create VertexId EdgeId VertexId triplets based on
    /// a traversal direction, where the vertex_id function argument appears in
    /// the first slot and the terminal vertex id appears in the final slot
    /// of each result triplet.
    fn incident_triplets(
        &self,
        vertex_id: VertexId,
        direction: Direction,
    ) -> Result<Vec<(VertexId, EdgeId, VertexId)>, GraphError> {
        let edge_ids = self.incident_edges(vertex_id, direction)?;
        let mut result: Vec<(VertexId, EdgeId, VertexId)> = Vec::with_capacity(edge_ids.len());
        for edge_id in edge_ids {
            let terminal_vid = self.incident_vertex(edge_id, direction)?;
            result.push((vertex_id, edge_id, terminal_vid));
        }
        Ok(result)
    }

    fn incident_triplet_attributes(
        &self,
        vertex_id: VertexId,
        direction: Direction,
    ) -> Result<Vec<(&Vertex, &Edge, &Vertex)>, GraphError> {
        let triplets = self.incident_triplets(vertex_id, direction)?;
        let mut result: Vec<(&Vertex, &Edge, &Vertex)> = Vec::with_capacity(triplets.len());
        for (src_id, edge_id, dst_id) in triplets {
            let src = self.vertex_attr(src_id)?;
            let edge = self.edge_attr(edge_id)?;
            let dst = self.vertex_attr(dst_id)?;
            result.push((src, edge, dst));
        }
        Ok(result)
    }
}

impl TryFrom<GraphConfig> for Graph {
    type Error = GraphError;

    /// tries to build a Graph from a GraphConfig.
    ///
    /// for both edge and vertex lists, we assume all ids can be used as indices
    /// to an array data structure. to find the size of each array, we pass once
    /// through each file to count the number of rows (minus header) of the CSV.
    /// then we can build a Vec *once* and insert rows as we decode them without
    /// a sort.
    fn try_from(config: GraphConfig) -> Result<Self, Self::Error> {
        info!("checking file length of edge and vertex input files");
        let (n_edges, n_vertices) = config.read_file_sizes()?;
        info!(
            "creating data structures to hold {} edges, {} vertices",
            n_edges, n_vertices
        );

        info!("reading edge list");

        let e_conf = EdgeLoaderConfig {
            config: &config,
            n_edges,
            n_vertices,
        };
        let e_result = TomTomEdgeList::try_from(e_conf)?;

        info!("reading vertex list");
        let v_conf = VertexLoaderConfig {
            config: &config,
            n_vertices,
        };
        let vertices: Vec<Vertex> = v_conf.try_into()?;

        let graph = GraphGraph {
            adj: e_result.adj,
            rev: e_result.rev,
            edges: e_result.edges,
            vertices,
        };

        Ok(graph)
    }
}
