mod error;
mod expiry;
mod multimap;
mod parsers;

use std::{
    collections::HashMap,
    fs::File,
    io::BufReader,
    path::{Path, PathBuf},
};

use dot_writer::{Attributes, DotWriter, NodeId};
use error::Error;
use eui48::MacAddress;
use expiry::ExpireSet;
use multimap::MultiMap;
use parsers::{DevicePoller, PortPoller};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct PortConfig {
    pub id: String,
    pub name: Option<String>,
    #[serde(default)]
    pollers: Vec<PortPoller>,
}

#[derive(Deserialize, Default, Clone)]
#[serde(rename = "lowercase")]
pub enum DeviceType {
    Router,
    Switch,
    Modem,
    AP,
    #[default]
    Unknown,
}

#[derive(Deserialize)]
pub struct DeviceConfig {
    pub id: String,
    pub name: Option<String>,
    #[serde(default, rename = "type")]
    pub device_type: DeviceType,
    pub mac: Vec<MacAddress>,
    #[serde(default)]
    pub ports: Vec<PortConfig>,
    #[serde(default)]
    pollers: Vec<DevicePoller>,
}

#[derive(Deserialize)]
pub struct NetworkConfig {
    pub devices: Vec<DeviceConfig>,
}

#[derive(Clone)]
pub struct Port {
    pub name: String,
    visible: ExpireSet<MacAddress>,
}

impl Port {
    fn from_config(config: &PortConfig) -> Self {
        Port {
            name: config.name.clone().unwrap_or_else(|| config.id.clone()),
            visible: Default::default(),
        }
    }

    fn can_see(&self, mac: &Vec<MacAddress>) -> bool {
        for mac in mac {
            if self.visible.contains(mac) {
                return true;
            }
        }
        false
    }
}

#[derive(Clone)]
pub struct Device {
    pub id: String,
    pub name: Option<String>,
    pub mac: Vec<MacAddress>,
    pub ports: HashMap<String, Port>,
}

impl Device {
    fn name(&self) -> &str {
        self.name.as_ref().unwrap_or(&self.id)
    }

    fn from_config(config: &DeviceConfig) -> Self {
        Device {
            id: config.id.clone(),
            name: config.name.clone(),
            mac: config.mac.clone(),
            ports: config
                .ports
                .iter()
                .map(|c| (c.id.clone(), Port::from_config(c)))
                .collect(),
        }
    }
}

pub struct Network {
    root: PathBuf,
    config: NetworkConfig,
    devices: MultiMap<MacAddress, Device>,
}

impl Network {
    pub fn new(config: NetworkConfig, root: &Path) -> Self {
        Network {
            root: root.to_owned(),
            devices: config
                .devices
                .iter()
                .map(|c| (c.mac.clone(), Device::from_config(c)))
                .collect(),
            config,
        }
    }

    pub fn poll(&mut self) -> Result<(), Error> {
        for device_config in self.config.devices.iter() {
            let device = self
                .devices
                .get_mut(device_config.mac.first().unwrap())
                .unwrap();

            for port_config in device_config.ports.iter() {
                let port = device.ports.get_mut(&port_config.id).unwrap();
                port.visible.expire();

                for poller in port_config.pollers.iter() {
                    let visible = poller.poll(&self.root)?;
                    port.visible.extend_from(visible);
                }
            }

            for poller in device_config.pollers.iter() {
                let visible = poller.poll(&self.root)?;
                for (port, visible) in visible {
                    if let Some(port) = device.ports.get_mut(&port) {
                        port.visible.extend_from(visible);
                    }
                }
            }
        }

        Ok(())
    }

    pub fn map(&self) -> String {
        let mut output = Vec::new();
        {
            let mut writer = DotWriter::from(&mut output);
            let mut graph = writer.graph();

            // First pass cleans up the visible lists to only the most adjacent.
            let mut devices = self.devices.clone();
            for device in devices.values_mut() {
                for port in device.ports.values_mut() {
                    let visible = port.visible.clone();
                    for mac in visible.iter() {
                        if let Some(other) = self.devices.get(mac) {
                            for other_port in other.ports.values() {
                                if !other_port.can_see(&device.mac) {
                                    for mac in other_port.visible.iter() {
                                        port.visible.remove(mac);
                                    }
                                }
                            }
                        }
                    }
                }
            }

            let mut device_nodes: HashMap<String, NodeId> = HashMap::new();
            let mut port_nodes: HashMap<(String, String), NodeId> = HashMap::new();

            // Now generate all the device nodes.
            for device in devices.values() {
                if device.ports.values().any(|p| !p.visible.is_empty()) {
                    let mut cluster = graph.cluster();
                    let device_id = {
                        let mut node = cluster.node_auto();
                        node.set_label(device.name());
                        device_nodes.insert(device.id.clone(), node.id());
                        node.id()
                    };

                    for (port_id, port) in device.ports.iter() {
                        if !port.visible.is_empty() {
                            let port_id = {
                                let mut node = cluster.node_auto();
                                node.set_label(&port.name);
                                node.set("shape", "point", false);
                                port_nodes.insert((device.id.clone(), port_id.clone()), node.id());
                                node.id()
                            };
                            cluster.edge(device_id.clone(), port_id);
                        }
                    }
                } else {
                    let mut node = graph.node_auto();
                    node.set_label(device.name());
                    device_nodes.insert(device.id.clone(), node.id());
                }
            }

            // Final pass lists all the connections
            devices.visit_pairs(|left, right| {
                let left_port = left
                    .ports
                    .iter()
                    .find(|(_i, p)| p.can_see(&right.mac))
                    .map(|(id, _port)| id);
                let right_port = right
                    .ports
                    .iter()
                    .find(|(_i, p)| p.can_see(&left.mac))
                    .map(|(id, _port)| id);

                let left_node = match left_port {
                    None => device_nodes.get(&left.id).unwrap(),
                    Some(port_id) => port_nodes.get(&(left.id.clone(), port_id.clone())).unwrap(),
                };

                let right_node = match right_port {
                    None => device_nodes.get(&right.id).unwrap(),
                    Some(port_id) => port_nodes
                        .get(&(right.id.clone(), port_id.clone()))
                        .unwrap(),
                };

                graph.edge(left_node, right_node);
            });

            for device in devices.iter() {
                for (id, port) in device.ports.iter() {
                    if port.visible.is_empty() {
                        continue;
                    }

                    let port_node = port_nodes.get(&(device.id.clone(), id.clone())).unwrap();
                    let device_count = port
                        .visible
                        .iter()
                        .filter(|m| !devices.contains_key(m))
                        .count();

                    if device_count == 0 {
                        continue;
                    }

                    let other_node_id = {
                        let mut other_node = graph.node_auto();
                        other_node.set_label(&format!("{} devices", device_count));
                        other_node.id()
                    };

                    graph.edge(port_node, other_node_id);
                }
            }
        }

        String::from_utf8(output).unwrap()
    }
}

impl TryFrom<&Path> for Network {
    type Error = Error;

    fn try_from(config_file: &Path) -> Result<Self, Self::Error> {
        let file = File::open(config_file).map_err(Error::IoError)?;
        let reader = BufReader::new(file);

        let config: NetworkConfig = serde_json::from_reader(reader).map_err(Error::ParseError)?;

        Ok(Network::new(config, config_file.parent().unwrap()))
    }
}
