use ana_tools::feature_flags::{FeatureFlagService, FlagValue};
use std::collections::HashMap;

pub struct InMemoryFeatureFlagService {
    flag_map: HashMap<String, FlagValue>,
}

impl InMemoryFeatureFlagService {
    pub fn new() -> Self {
        Self {
            flag_map: HashMap::new(),
        }
    }

    pub fn with_flag(mut self, flag_name: String, flag_value: FlagValue) -> Self {
        self.flag_map.insert(flag_name, flag_value);
        self
    }
}

impl Default for InMemoryFeatureFlagService {
    fn default() -> Self {
        Self::new()
    }
}

impl FeatureFlagService for InMemoryFeatureFlagService {
    fn get_flag_value(
        &self,
        key: &str,
        _context_key: Option<String>,
        default_value: FlagValue,
    ) -> FlagValue {
        let key = String::from(key);
        if self.flag_map.contains_key(&key) {
            self.flag_map[&key].clone()
        } else {
            default_value
        }
    }

    fn get_flag_multicontext_value(
        &self,
        _key_flag: &str,
        _key_user: &str,
        _context_keys: Option<HashMap<String, String>>,
        _default_value: FlagValue,
    ) -> FlagValue {
        false.into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handle_in_memory_service_creation() {
        let service = InMemoryFeatureFlagService::new()
            .with_flag("test-flag".to_owned(), true.into())
            .with_flag("test-flag-2".to_owned(), false.into());

        assert!(service
            .get_flag_value("test-flag", None, false.into())
            .as_bool()
            .unwrap());

        assert!(!service
            .get_flag_value("test-flag-2", None, true.into())
            .as_bool()
            .unwrap());
    }
}
