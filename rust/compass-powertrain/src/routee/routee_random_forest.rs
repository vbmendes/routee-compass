use std::sync::Arc;

use compass_core::model::property::edge::Edge;
use compass_core::model::property::vertex::Vertex;
use compass_core::model::traversal::default::velocity_lookup::VelocityLookupModel;
use compass_core::model::traversal::state::state_variable::StateVar;
use compass_core::model::traversal::state::traversal_state::TraversalState;
use compass_core::model::traversal::traversal_model::TraversalModel;
use compass_core::model::traversal::traversal_model_error::TraversalModelError;
use compass_core::model::traversal::traversal_result::TraversalResult;
use compass_core::model::units::{EnergyUnit, TimeUnit};
use compass_core::model::{cost::cost::Cost, units::Velocity};
use compass_core::util::geo::haversine::coord_distance_km;
use smartcore::{
    ensemble::random_forest_regressor::RandomForestRegressor, linalg::basic::matrix::DenseMatrix,
};
use uom::si;

pub struct RouteERandomForestModel {
    pub velocity_model: Arc<VelocityLookupModel>,
    pub routee_model: RandomForestRegressor<f64, f64, DenseMatrix<f64>, Vec<f64>>,
    pub energy_unit: EnergyUnit,
    pub minimum_energy_per_mile: f64,
}

impl TraversalModel for RouteERandomForestModel {
    fn initial_state(&self) -> TraversalState {
        vec![StateVar(0.0)]
    }
    fn cost_estimate(
        &self,
        src: &Vertex,
        dst: &Vertex,
        _state: &TraversalState,
    ) -> Result<Cost, TraversalModelError> {
        let distance = coord_distance_km(src.coordinate, dst.coordinate)
            .map_err(TraversalModelError::NumericError)?;
        let distance_miles = distance.get::<si::length::mile>();
        let minimum_energy = match self.energy_unit {
            EnergyUnit::GallonsGasoline => distance_miles * self.minimum_energy_per_mile,
        };
        Ok(Cost::from(minimum_energy))
    }
    fn traversal_cost(
        &self,
        src: &Vertex,
        edge: &Edge,
        dst: &Vertex,
        state: &TraversalState,
    ) -> Result<TraversalResult, TraversalModelError> {
        let speed_result = self.velocity_model.traversal_cost(src, edge, dst, state)?;
        let speed_kph: f64 = speed_result.total_cost.into();
        let distance = edge.distance;
        let grade = edge.grade;
        let distance_mile = distance.get::<si::length::mile>();
        let grade_percent = grade.get::<si::ratio::percent>();
        let speed_mph = Velocity::new::<si::velocity::kilometer_per_hour>(speed_kph.into())
            .get::<si::velocity::mile_per_hour>();
        let x = DenseMatrix::from_2d_vec(&vec![vec![speed_mph, grade_percent]]);
        let energy_per_mile = self
            .routee_model
            .predict(&x)
            .map_err(|e| TraversalModelError::PredictionModel(e.to_string()))?;
        let mut energy_cost = energy_per_mile[0] * distance_mile;

        // set cost to zero if it's negative since we can't currently handle negative costs
        energy_cost = if energy_cost < 0.0 { 0.0 } else { energy_cost };

        let mut updated_state = state.clone();
        updated_state[0] = state[0] + StateVar(energy_cost);
        let result = TraversalResult {
            total_cost: Cost::from(energy_cost),
            updated_state,
        };
        Ok(result)
    }
    fn summary(&self, state: &TraversalState) -> serde_json::Value {
        let total_energy = state[0].0;
        let energy_units = match self.energy_unit {
            EnergyUnit::GallonsGasoline => "gallons_gasoline",
        };
        serde_json::json!({
            "total_energy": total_energy,
            "energy_units": energy_units
        })
    }
}

impl RouteERandomForestModel {
    pub fn new(
        velocity_model: Arc<VelocityLookupModel>,
        routee_model_path: &String,
        energy_unit: EnergyUnit,
    ) -> Result<Self, TraversalModelError> {
        // Load random forest binary file
        let rf_binary = std::fs::read(routee_model_path.clone()).map_err(|e| {
            TraversalModelError::FileReadError(routee_model_path.clone(), e.to_string())
        })?;
        let rf: RandomForestRegressor<f64, f64, DenseMatrix<f64>, Vec<f64>> =
            bincode::deserialize(&rf_binary).map_err(|e| {
                TraversalModelError::FileReadError(routee_model_path.clone(), e.to_string())
            })?;

        // sweep a fixed set of speed and grade values to find the minimum energy per mile rate from the incoming rf model
        let mut minimum_energy_per_mile = std::f64::MAX;

        let start_time = std::time::Instant::now();

        for speed_mph in 1..100 {
            for grade_percent in -20..20 {
                let x =
                    DenseMatrix::from_2d_vec(&vec![vec![speed_mph as f64, grade_percent as f64]]);
                let energy_per_mile = rf
                    .predict(&x)
                    .map_err(|e| TraversalModelError::PredictionModel(e.to_string()))?;
                if energy_per_mile[0] < minimum_energy_per_mile {
                    minimum_energy_per_mile = energy_per_mile[0];
                }
            }
        }

        let end_time = std::time::Instant::now();
        let search_time = end_time - start_time;

        log::debug!(
            "found minimum_energy_per_mile: {} for {} in {} milliseconds",
            minimum_energy_per_mile,
            routee_model_path,
            search_time.as_millis()
        );

        Ok(RouteERandomForestModel {
            velocity_model,
            routee_model: rf,
            energy_unit,
            minimum_energy_per_mile,
        })
    }

    pub fn new_w_speed_file(
        speed_file: &String,
        routee_model_path: &String,
        time_unit: TimeUnit,
        energy_rate_unit: EnergyUnit,
    ) -> Result<Self, TraversalModelError> {
        let velocity_model = VelocityLookupModel::from_file(&speed_file, time_unit)?;
        Self::new(
            Arc::new(velocity_model),
            routee_model_path,
            energy_rate_unit,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use compass_core::model::units::{Length, Ratio};
    use compass_core::model::{
        graph::{edge_id::EdgeId, vertex_id::VertexId},
        property::{edge::Edge, road_class::RoadClass, vertex::Vertex},
    };
    use geo::coord;
    use std::path::PathBuf;
    use uom::si;

    #[test]
    fn test_edge_cost_lookup_from_file() {
        let speed_file_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("routee")
            .join("test")
            .join("velocities.txt");
        let model_file_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("routee")
            .join("test")
            .join("Toyota_Camry.bin");
        let speed_file_name = speed_file_path.to_str().unwrap();
        let model_file_name = model_file_path.to_str().unwrap();
        let v = Vertex {
            vertex_id: VertexId(0),
            coordinate: coord! {x: -86.67, y: 36.12},
        };
        fn mock_edge(edge_id: usize) -> Edge {
            return Edge {
                edge_id: EdgeId(edge_id as u64),
                src_vertex_id: VertexId(0),
                dst_vertex_id: VertexId(1),
                road_class: RoadClass(2),
                distance: Length::new::<si::length::meter>(100.0),
                grade: Ratio::new::<si::ratio::per_mille>(0.0),
            };
        }
        let speed_file = String::from(speed_file_name);
        let routee_model_path = String::from(model_file_name);
        let rf_predictor = RouteERandomForestModel::new_w_speed_file(
            &speed_file,
            &routee_model_path,
            TimeUnit::Seconds,
            EnergyUnit::GallonsGasoline,
        )
        .unwrap();
        let initial = rf_predictor.initial_state();
        let e1 = mock_edge(0);
        // 100 meters @ 10kph should take 36 seconds ((0.1/10) * 3600)
        let result = rf_predictor.traversal_cost(&v, &e1, &v, &initial).unwrap();
    }
}