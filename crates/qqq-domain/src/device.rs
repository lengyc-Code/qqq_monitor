#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeviceKind {
    System,
    Cpu,
    Core,
    Memory,
    Disk,
    Volume,
    Network,
    Gpu,
    /// Frames of the application in the foreground, named after its executable.
    Frames,
}

#[derive(Clone, Debug)]
pub struct DeviceDescriptor {
    pub id: String,
    /// Optional identity safe to save across application sessions, e.g. an interface GUID.
    pub persistent_id: Option<String>,
    /// The system's own name, such as the CPU brand or adapter description; empty for
    /// devices the system leaves unnamed, which the GUI names by kind and index.
    pub name: String,
    pub kind: DeviceKind,
    /// Remote host the device belongs to; `None` for this computer.
    pub host: Option<String>,
    pub online: bool,
    /// Pairs of a stable key such as `volume_label` and a value; enumerated values are
    /// stable tokens too, such as `physical` for `interface_type`.
    pub details: Vec<(String, String)>,
}

impl DeviceDescriptor {
    /// Identity that outlasts this session: the persistent ID when there is one, else the ID.
    pub fn identity(&self) -> &str {
        self.persistent_id.as_deref().unwrap_or(&self.id)
    }
    pub fn new(id: impl Into<String>, name: impl Into<String>, kind: DeviceKind) -> Self {
        Self {
            id: id.into(),
            persistent_id: None,
            name: name.into(),
            kind,
            host: None,
            online: true,
            details: Vec::new(),
        }
    }
    pub fn persistent_id(mut self, id: impl Into<String>) -> Self {
        self.persistent_id = Some(id.into());
        self
    }
    pub fn detail(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.details.push((key.into(), value.into()));
        self
    }
}
