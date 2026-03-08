use std::collections::BTreeMap;

use oxvba_compiler::ProjectEventDispatchBinding;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct EventSourceKey {
    pub project_name: String,
    pub module_name: String,
    pub event_name: String,
}

impl EventSourceKey {
    pub fn new(
        project_name: impl Into<String>,
        module_name: impl Into<String>,
        event_name: impl Into<String>,
    ) -> Self {
        Self {
            project_name: normalize(project_name.into()),
            module_name: normalize(module_name.into()),
            event_name: normalize(event_name.into()),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct EventDispatcher {
    handlers: BTreeMap<EventSourceKey, Vec<String>>,
}

impl EventDispatcher {
    pub fn clear(&mut self) {
        self.handlers.clear();
    }

    pub fn apply_bindings(&mut self, bindings: &[ProjectEventDispatchBinding]) {
        self.handlers.clear();
        for binding in bindings {
            self.subscribe(
                &EventSourceKey::new(
                    &binding.source_project_name,
                    &binding.source_module_name,
                    &binding.event_name,
                ),
                binding.handler_symbol.clone(),
            );
        }
    }

    pub fn subscribe(&mut self, source: &EventSourceKey, handler_symbol: impl Into<String>) {
        let handler = normalize(handler_symbol.into());
        let entry = self.handlers.entry(source.clone()).or_default();
        if !entry.iter().any(|existing| existing == &handler) {
            entry.push(handler);
        }
    }

    pub fn unsubscribe(&mut self, source: &EventSourceKey, handler_symbol: &str) -> bool {
        let Some(entry) = self.handlers.get_mut(source) else {
            return false;
        };
        let handler = normalize(handler_symbol.to_string());
        let initial_len = entry.len();
        entry.retain(|existing| existing != &handler);
        let removed = entry.len() != initial_len;
        if entry.is_empty() {
            self.handlers.remove(source);
        }
        removed
    }

    pub fn dispatch(&self, source: &EventSourceKey) -> Vec<String> {
        self.handlers.get(source).cloned().unwrap_or_default()
    }
}

fn normalize(value: String) -> String {
    value.trim().to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::{EventDispatcher, EventSourceKey};
    use oxvba_compiler::ProjectEventDispatchBinding;

    #[test]
    fn dispatcher_applies_bindings_and_dispatches_in_stable_order() {
        let mut dispatcher = EventDispatcher::default();
        dispatcher.apply_bindings(&[
            ProjectEventDispatchBinding {
                source_project_name: "ProjectA".to_string(),
                source_module_name: "Emitter".to_string(),
                event_name: "Changed".to_string(),
                handler_symbol: "pmr_projecta_sinka_changed".to_string(),
            },
            ProjectEventDispatchBinding {
                source_project_name: "ProjectA".to_string(),
                source_module_name: "Emitter".to_string(),
                event_name: "Changed".to_string(),
                handler_symbol: "pmr_projecta_sinkb_changed".to_string(),
            },
        ]);

        let handlers = dispatcher.dispatch(&EventSourceKey::new("ProjectA", "Emitter", "Changed"));
        assert_eq!(
            handlers,
            vec![
                "pmr_projecta_sinka_changed".to_string(),
                "pmr_projecta_sinkb_changed".to_string()
            ]
        );
    }

    #[test]
    fn dispatcher_unsubscribe_removes_handler_and_cleans_empty_source() {
        let mut dispatcher = EventDispatcher::default();
        let source = EventSourceKey::new("ProjectA", "Emitter", "Changed");
        dispatcher.subscribe(&source, "pmr_projecta_sinka_changed");
        assert!(dispatcher.unsubscribe(&source, "pmr_projecta_sinka_changed"));
        assert!(dispatcher.dispatch(&source).is_empty());
    }

    #[test]
    fn dispatcher_subscribe_deduplicates_case_insensitively() {
        let mut dispatcher = EventDispatcher::default();
        let source = EventSourceKey::new("ProjectA", "Emitter", "Changed");
        dispatcher.subscribe(&source, "pmr_projecta_sinka_changed");
        dispatcher.subscribe(&source, "PMR_PROJECTA_SINKA_CHANGED");

        let handlers = dispatcher.dispatch(&source);
        assert_eq!(handlers, vec!["pmr_projecta_sinka_changed".to_string()]);
    }
}
