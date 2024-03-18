use super::internal_state_model_ops::InternalStateModelOps;
use super::{
    custom_feature_format::CustomFeatureFormat, indexed_state_feature::IndexedStateFeature,
    state_error::StateError, state_feature::StateFeature, update_operation::UpdateOperation,
};
use crate::model::{
    traversal::state::state_variable::StateVar,
    unit::{Distance, DistanceUnit, Energy, EnergyUnit, Time, TimeUnit},
};
use itertools::Itertools;
use serde_json::json;
use std::collections::{HashMap, HashSet};
use std::iter::Enumerate;

/// a state model tracks information about each feature in a search state vector.
/// in concept, it is modeled as a mapping from a feature_name String to a StateFeature
/// object (see NFeatures, below). there are 4 additional implementations that specialize
/// for the case where fewer than 5 features are required in order to improve CPU performance.
pub enum StateModel {
    OneFeature {
        key: String,
        value: StateFeature,
    },
    TwoFeatures {
        k1: String,
        k2: String,
        v1: StateFeature,
        v2: StateFeature,
    },
    ThreeFeatures {
        k1: String,
        k2: String,
        k3: String,
        v1: StateFeature,
        v2: StateFeature,
        v3: StateFeature,
    },
    FourFeatures {
        k1: String,
        k2: String,
        k3: String,
        k4: String,
        v1: StateFeature,
        v2: StateFeature,
        v3: StateFeature,
        v4: StateFeature,
    },
    NFeatures(HashMap<String, IndexedStateFeature>),
}

type FeatureIterator<'a> = Box<dyn Iterator<Item = (&'a String, &'a StateFeature)> + 'a>;
type IndexedFeatureIterator<'a> =
    Enumerate<Box<dyn Iterator<Item = (&'a String, &'a StateFeature)> + 'a>>;

impl StateModel {
    pub fn empty() -> StateModel {
        StateModel::NFeatures(HashMap::new())
    }

    pub fn new(features: Vec<(String, StateFeature)>) -> StateModel {
        use StateModel as S;
        let sorted = features
            .into_iter()
            .sorted_by_key(|(n, _)| n.clone())
            .collect::<Vec<_>>();

        match &sorted[..] {
            [] => S::empty(),
            [(key, value)] => S::OneFeature {
                key: key.clone(),
                value: value.clone(),
            },
            [(k1, v1), (k2, v2)] => S::TwoFeatures {
                k1: k1.clone(),
                k2: k2.clone(),
                v1: v1.clone(),
                v2: v2.clone(),
            },
            [(k1, v1), (k2, v2), (k3, v3)] => S::ThreeFeatures {
                k1: k1.clone(),
                k2: k2.clone(),
                k3: k3.clone(),
                v1: v1.clone(),
                v2: v2.clone(),
                v3: v3.clone(),
            },
            [(k1, v1), (k2, v2), (k3, v3), (k4, v4)] => S::FourFeatures {
                k1: k1.clone(),
                k2: k2.clone(),
                k3: k3.clone(),
                k4: k4.clone(),
                v1: v1.clone(),
                v2: v2.clone(),
                v3: v3.clone(),
                v4: v4.clone(),
            },
            _ => {
                let indexed = sorted
                    .into_iter()
                    .enumerate()
                    .map(|(index, (feature_name, feature))| {
                        let indexed_feature = IndexedStateFeature {
                            index,
                            feature: feature.clone(),
                        };
                        (feature_name.clone(), indexed_feature)
                    })
                    .collect::<HashMap<_, _>>();
                S::NFeatures(indexed)
            }
        }
    }

    /// extends a state model by adding additional key/value pairs to the model mapping.
    /// in the case of name collision, a warning is logged to the user and the newer
    /// variable is used.
    ///
    /// this method is used when state models are updated by the user query as Services
    /// become Models in the SearchApp.
    ///
    /// # Arguments
    /// * `query` - JSON search query contents containing state model information
    pub fn extend(&self, entries: Vec<(String, StateFeature)>) -> Result<StateModel, StateError> {
        let overwrite_keys = entries
            .iter()
            .flat_map(|(feature_name, state_feature)| {
                if let Ok(old_feature) = self.get_feature(feature_name) {
                    log::warn!(
                        "user overwriting state model feature {}.\nold: {}\nnew: {}",
                        feature_name,
                        state_feature,
                        old_feature
                    );
                    Some(feature_name.clone())
                } else {
                    None
                }
            })
            .collect::<HashSet<_>>();

        let all_features = self
            .iter()
            .filter(|(n, _)| !overwrite_keys.contains(*n))
            .map(|(n, f)| (n.clone(), f.clone()))
            .chain(entries)
            .collect::<Vec<_>>();

        Ok(all_features.into())
    }

    pub fn len(&self) -> usize {
        match self {
            StateModel::OneFeature { .. } => 1,
            StateModel::TwoFeatures { .. } => 2,
            StateModel::ThreeFeatures { .. } => 3,
            StateModel::FourFeatures { .. } => 4,
            StateModel::NFeatures(f) => f.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn contains_key(&self, k: &str) -> bool {
        self.get_index(k).is_ok()
    }

    /// collects the state model tuples and clones them so they can
    /// be used to build other collections
    pub fn to_vec(&self) -> Vec<(String, IndexedStateFeature)> {
        self.iter()
            .enumerate()
            .map(|(idx, (n, f))| {
                (
                    n.clone(),
                    IndexedStateFeature {
                        index: idx,
                        feature: f.clone(),
                    },
                )
            })
            .collect_vec()
    }

    /// iterates over the features in this state in their state vector index ordering.
    pub fn iter(&self) -> FeatureIterator {
        let iter = StateModelIter {
            iterable: self,
            index: 0,
        };
        Box::new(iter)
    }

    /// iterator that includes the state vector index along with the feature name and StateFeature
    pub fn indexed_iter(&self) -> IndexedFeatureIterator {
        self.iter().enumerate()
    }

    /// Creates the initial state of a search. this should be a vector of
    /// accumulators, defined in the state model configuration.
    ///
    /// # Returns
    ///
    /// an initialized, "zero"-valued traversal state, or an error
    pub fn initial_state(&self) -> Result<Vec<StateVar>, StateError> {
        self.iter()
            .map(|(_, feature)| {
                let initial = feature.get_initial()?;
                Ok(initial)
            })
            .collect::<Result<Vec<_>, _>>()
    }

    /// retrieves a state variable that is expected to have a type of Distance
    ///
    /// # Arguments
    /// * `state` - state vector to inspect
    /// * `name`  - feature name to extract
    /// * `unit`  - feature is converted to this unit before returning
    ///
    /// # Returns
    ///
    /// feature value in the expected unit type, or an error
    pub fn get_distance(
        &self,
        state: &[StateVar],
        name: &str,
        unit: &DistanceUnit,
    ) -> Result<Distance, StateError> {
        let value = self.get_value(state, name)?;
        let feature = self.get_feature(name)?;
        let result = feature.get_distance_unit()?.convert(&value.into(), unit);
        Ok(result)
    }
    /// retrieves a state variable that is expected to have a type of Time
    ///
    /// # Arguments
    /// * `state` - state vector to inspect
    /// * `name`  - feature name to extract
    /// * `unit`  - feature is converted to this unit before returning
    ///
    /// # Returns
    ///
    /// feature value in the expected unit type, or an error
    pub fn get_time(
        &self,
        state: &[StateVar],
        name: &str,
        unit: &TimeUnit,
    ) -> Result<Time, StateError> {
        let value = self.get_value(state, name)?;
        let feature = self.get_feature(name)?;
        let result = feature.get_time_unit()?.convert(&value.into(), unit);
        Ok(result)
    }
    /// retrieves a state variable that is expected to have a type of Energy
    ///
    /// # Arguments
    /// * `state` - state vector to inspect
    /// * `name`  - feature name to extract
    /// * `unit`  - feature is converted to this unit before returning
    ///
    /// # Returns
    ///
    /// feature value in the expected unit type, or an error
    pub fn get_energy(
        &self,
        state: &[StateVar],
        name: &str,
        unit: &EnergyUnit,
    ) -> Result<Energy, StateError> {
        let value = self.get_value(state, name)?;
        let feature = self.get_feature(name)?;
        let result = feature.get_energy_unit()?.convert(&value.into(), unit);
        Ok(result)
    }
    /// retrieves a state variable that is expected to have a type of f64.
    ///
    /// # Arguments
    /// * `state` - state vector to inspect
    /// * `name`  - feature name to extract
    ///
    /// # Returns
    ///
    /// the expected value or an error
    pub fn get_custom_f64(&self, state: &[StateVar], name: &str) -> Result<f64, StateError> {
        let (value, format) = self.get_custom_state_variable(state, name)?;
        let result = format.decode_f64(&value)?;
        Ok(result)
    }
    /// retrieves a state variable that is expected to have a type of i64.
    ///
    /// # Arguments
    /// * `state` - state vector to inspect
    /// * `name`  - feature name to extract
    ///
    /// # Returns
    ///
    /// the expected value or an error
    pub fn get_custom_i64(&self, state: &[StateVar], name: &str) -> Result<i64, StateError> {
        let (value, format) = self.get_custom_state_variable(state, name)?;
        let result = format.decode_i64(&value)?;
        Ok(result)
    }
    /// retrieves a state variable that is expected to have a type of u64.
    ///
    /// # Arguments
    /// * `state` - state vector to inspect
    /// * `name`  - feature name to extract
    ///
    /// # Returns
    ///
    /// the expected value or an error
    pub fn get_custom_u64(&self, state: &[StateVar], name: &str) -> Result<u64, StateError> {
        let (value, format) = self.get_custom_state_variable(state, name)?;
        let result = format.decode_u64(&value)?;
        Ok(result)
    }
    /// retrieves a state variable that is expected to have a type of bool.
    ///
    /// # Arguments
    /// * `state` - state vector to inspect
    /// * `name`  - feature name to extract
    ///
    /// # Returns
    ///
    /// the expected value or an error
    pub fn get_custom_bool(&self, state: &[StateVar], name: &str) -> Result<bool, StateError> {
        let (value, format) = self.get_custom_state_variable(state, name)?;
        let result = format.decode_bool(&value)?;
        Ok(result)
    }

    /// internal helper function that retrieves a value as a feature vector state variable
    /// along with the custom feature's format. this is used by the four specialized get_custom
    /// methods for specific types.
    ///
    /// # Arguments
    /// * `state` - state vector to inspect
    /// * `name`  - feature name to extract
    ///
    /// # Returns
    ///
    /// the expected value as a state variable (not decoded) or an error
    fn get_custom_state_variable(
        &self,
        state: &[StateVar],
        name: &str,
    ) -> Result<(StateVar, &CustomFeatureFormat), StateError> {
        let value = self.get_value(state, name)?;
        let feature = self.get_feature(name)?;
        let format = feature.get_custom_feature_format()?;
        Ok((value, format))
    }

    /// gets the difference from some previous value to some next value by name.
    ///
    /// # Arguments
    ///
    /// * `prev` - the previous state to inspect
    /// * `next` - the next state to inspect
    /// * `name`  - name of feature to compare
    ///
    /// # Result
    ///
    /// the delta between states for this variable, or an error
    pub fn get_delta(
        &self,
        prev: &[StateVar],
        next: &[StateVar],
        name: &str,
    ) -> Result<StateVar, StateError> {
        let prev_val = self.get_value(prev, name)?;
        let next_val = self.get_value(next, name)?;
        Ok(next_val - prev_val)
    }

    /// adds a distance value with distance unit to this feature vector
    pub fn add_distance(
        &self,
        state: &mut [StateVar],
        name: &str,
        distance: &Distance,
        from_unit: &DistanceUnit,
    ) -> Result<(), StateError> {
        let prev_distance = self.get_distance(state, name, from_unit)?;
        let next_distance = prev_distance + *distance;
        self.set_distance(state, name, &next_distance, from_unit)
    }

    /// adds a time value with time unit to this feature vector
    pub fn add_time(
        &self,
        state: &mut [StateVar],
        name: &str,
        time: &Time,
        from_unit: &TimeUnit,
    ) -> Result<(), StateError> {
        let prev_time = self.get_time(state, name, from_unit)?;
        let next_time = prev_time + *time;
        self.set_time(state, name, &next_time, from_unit)
    }

    /// adds a energy value with energy unit to this feature vector
    pub fn add_energy(
        &self,
        state: &mut [StateVar],
        name: &str,
        energy: &Energy,
        from_unit: &EnergyUnit,
    ) -> Result<(), StateError> {
        let prev_energy = self.get_energy(state, name, from_unit)?;
        let next_energy = prev_energy + *energy;
        self.set_energy(state, name, &next_energy, from_unit)
    }

    pub fn set_distance(
        &self,
        state: &mut [StateVar],
        name: &str,
        distance: &Distance,
        from_unit: &DistanceUnit,
    ) -> Result<(), StateError> {
        let feature = self.get_feature(name)?;
        let to_unit = feature.get_distance_unit()?;
        let value = from_unit.convert(distance, &to_unit);
        self.update_state(state, name, &value.into(), UpdateOperation::Replace)
    }

    pub fn set_time(
        &self,
        state: &mut [StateVar],
        name: &str,
        time: &Time,
        from_unit: &TimeUnit,
    ) -> Result<(), StateError> {
        let feature = self.get_feature(name)?;
        let to_unit = feature.get_time_unit()?;
        let value = from_unit.convert(time, &to_unit);
        self.update_state(state, name, &value.into(), UpdateOperation::Replace)
    }

    pub fn set_energy(
        &self,
        state: &mut [StateVar],
        name: &str,
        energy: &Energy,
        from_unit: &EnergyUnit,
    ) -> Result<(), StateError> {
        let feature = self.get_feature(name)?;
        let to_unit = feature.get_energy_unit()?;
        let value = from_unit.convert(energy, &to_unit);
        self.update_state(state, name, &value.into(), UpdateOperation::Replace)
    }

    pub fn set_custom_f64(
        &self,
        state: &mut [StateVar],
        name: &str,
        value: &f64,
    ) -> Result<(), StateError> {
        let feature = self.get_feature(name)?;
        let format = feature.get_custom_feature_format()?;
        let encoded_value = format.encode_f64(value)?;
        self.update_state(state, name, &encoded_value, UpdateOperation::Replace)
    }

    pub fn set_custom_i64(
        &self,
        state: &mut [StateVar],
        name: &str,
        value: &i64,
    ) -> Result<(), StateError> {
        let feature = self.get_feature(name)?;
        let format = feature.get_custom_feature_format()?;
        let encoded_value = format.encode_i64(value)?;
        self.update_state(state, name, &encoded_value, UpdateOperation::Replace)
    }

    pub fn set_custom_u64(
        &self,
        state: &mut [StateVar],
        name: &str,
        value: &u64,
    ) -> Result<(), StateError> {
        let feature = self.get_feature(name)?;
        let format = feature.get_custom_feature_format()?;
        let encoded_value = format.encode_u64(value)?;
        self.update_state(state, name, &encoded_value, UpdateOperation::Replace)
    }

    pub fn set_custom_bool(
        &self,
        state: &mut [StateVar],
        name: &str,
        value: &bool,
    ) -> Result<(), StateError> {
        let feature = self.get_feature(name)?;
        let format = feature.get_custom_feature_format()?;
        let encoded_value = format.encode_bool(value)?;
        self.update_state(state, name, &encoded_value, UpdateOperation::Replace)
    }

    /// uses the state model to pretty print a state instance as a JSON object
    ///
    /// # Arguments
    /// * `state` - any (valid) state vector instance
    ///
    /// # Result
    /// A JSON object representation of that vector
    pub fn serialize_state(&self, state: &[StateVar]) -> serde_json::Value {
        let output = self
            .iter()
            .zip(state.iter())
            .map(|((name, _), state_var)| (name, state_var))
            .collect::<HashMap<_, _>>();
        json![output]
    }

    /// uses the built-in serialization codec to output the state model representation as a JSON object
    pub fn serialize_state_model(&self) -> serde_json::Value {
        json![self.iter().collect::<HashMap<_, _>>()]
    }
}

pub struct StateModelIter<'a> {
    iterable: &'a StateModel,
    index: usize,
}

impl<'a> Iterator for StateModelIter<'a> {
    type Item = (&'a String, &'a StateFeature);

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.iterable.len() {
            return None;
        }
        if let Ok(tuple) = self.iterable.get(self.index) {
            self.index += 1;
            Some(tuple)
        } else {
            None
        }
    }
}

impl IntoIterator for StateModel {
    type Item = (String, IndexedStateFeature);

    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        match self {
            StateModel::OneFeature { key, value } => {
                vec![(key, IndexedStateFeature::new(0, value))].into_iter()
            }
            StateModel::TwoFeatures { k1, k2, v1, v2 } => vec![
                (k1, IndexedStateFeature::new(0, v1)),
                (k2, IndexedStateFeature::new(1, v2)),
            ]
            .into_iter(),
            StateModel::ThreeFeatures {
                k1,
                k2,
                k3,
                v1,
                v2,
                v3,
            } => vec![
                (k1, IndexedStateFeature::new(0, v1)),
                (k2, IndexedStateFeature::new(1, v2)),
                (k3, IndexedStateFeature::new(2, v3)),
            ]
            .into_iter(),
            StateModel::FourFeatures {
                k1,
                k2,
                k3,
                k4,
                v1,
                v2,
                v3,
                v4,
            } => vec![
                (k1, IndexedStateFeature::new(0, v1)),
                (k2, IndexedStateFeature::new(1, v2)),
                (k3, IndexedStateFeature::new(2, v3)),
                (k4, IndexedStateFeature::new(3, v4)),
            ]
            .into_iter(),
            StateModel::NFeatures(f) => f.into_iter().sorted_by_key(|(_, f)| f.index),
        }
    }
}

impl<'a> TryFrom<&'a serde_json::Value> for StateModel {
    type Error = StateError;

    /// builds a new state model from a JSON array of deserialized StateFeatures.
    /// the size of the JSON object matches the size of the feature vector. downstream
    /// models such as the TraversalModel can look up features by name and retrieve
    /// the codec or unit representation in order to do state vector arithmetic.
    ///
    /// # Example
    ///
    /// ### Deserialization
    ///
    /// an example TOML representation of a StateModel:
    ///
    /// ```toml
    /// [state]
    /// distance = { "distance_unit" = "kilometers", initial = 0.0 },
    /// time = { "time_unit" = "minutes", initial = 0.0 },
    /// battery_soc = { name = "soc", unit = "percent", format = { type = "floating_point", initial = 0.0 } }
    ///
    /// the same example as JSON (convert '=' into ':', and enquote object keys):
    ///
    /// ```json
    /// {
    ///   "distance": { "distance_unit": "kilometers", "initial": 0.0 },
    ///   "time": { "time_unit": "minutes", "initial": 0.0 },
    ///   "battery_soc": {
    ///     "name": "soc",
    ///     "unit": "percent",
    ///     "format": {
    ///       "type": "floating_point",
    ///       "initial": 0.0
    ///     }
    ///   }
    /// }
    /// ```
    fn try_from(json: &'a serde_json::Value) -> Result<StateModel, StateError> {
        let tuples = json
            .as_object()
            .ok_or_else(|| {
                StateError::BuildError(String::from(
                    "expected state model configuration to be a JSON object {}",
                ))
            })?
            .into_iter()
            .map(|(feature_name, feature_json)| {
                let feature = serde_json::from_value::<StateFeature>(feature_json.clone())
                    .map_err(|e| {
                        StateError::BuildError(format!(
                        "unable to parse state feature row with name '{}' contents '{}' due to: {}",
                        feature_name.clone(),
                        feature_json.clone(),
                        e
                    ))
                    })?;
                Ok((feature_name.clone(), feature))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let state_model = StateModel::from(tuples);
        Ok(state_model)
    }
}

impl From<Vec<(String, StateFeature)>> for StateModel {
    fn from(value: Vec<(String, StateFeature)>) -> Self {
        StateModel::new(value)
    }
}
