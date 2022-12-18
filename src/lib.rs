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

#[derive(Deserialize)]
pub struct DeviceConfig {
    pub id: String,
    pub name: Option<String>,
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
}

pub struct Device {
    pub name: String,
    pub mac: Vec<MacAddress>,
    pub ports: HashMap<String, Port>,
}

impl Device {
    fn from_config(config: &DeviceConfig) -> Self {
        Device {
            name: config.name.clone().unwrap_or_else(|| config.id.clone()),
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

    pub fn map(&self) {
        for device in self.devices.iter() {
            println!("Device {}", device.name);
            for port in device.ports.values() {
                println!("  Port {}", port.name);
                for visible in port.visible.iter() {
                    println!("    {}", visible);
                }
            }
        }
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
