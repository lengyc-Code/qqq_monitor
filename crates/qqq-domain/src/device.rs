#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeviceKind {
    System,
    Cpu,
    Core,
    Memory,
    Disk,
    Volume,
    Network,
}

#[derive(Clone, Debug)]
pub struct DeviceDescriptor {
    pub id: String,
    /// Optional identity safe to save across application sessions, e.g. an interface GUID.
    pub persistent_id: Option<String>,
    pub name: String,
    pub kind: DeviceKind,
    pub online: bool,
    pub details: Vec<(String, String)>,
}

impl DeviceDescriptor {
    pub fn new(id: impl Into<String>, name: impl Into<String>, kind: DeviceKind) -> Self {
        Self {
            id: id.into(),
            persistent_id: None,
            name: name.into(),
            kind,
            online: true,
            details: Vec::new(),
        }
    }
    pub fn persistent_id(mut self, id: impl Into<String>) -> Self {
        self.persistent_id = Some(id.into());
        self
    }
    pub fn detail(mut self, label: impl Into<String>, value: impl Into<String>) -> Self {
        self.details.push((label.into(), value.into()));
        self
    }
}
