pub mod metadata;
pub mod registry;

pub use registry::{ModelDescriptor, FieldDescriptor, FieldType, IdType, ModelRegistry};
pub use metadata::{ViewType, ActionType, MenuDescriptor, ViewDescriptor, ActionDescriptor};

pub trait ModuleRoutes: Send + Sync {
    fn name(&self) -> &str;

    fn route_descriptions(&self) -> Vec<RouteDescriptor>;
}

pub struct RouteDescriptor {
    pub method: &'static str,
    pub path: &'static str,
    pub handler: &'static str,
    pub min_role: &'static str,
}
