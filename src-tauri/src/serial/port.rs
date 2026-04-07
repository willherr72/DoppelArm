use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct PortInfo {
    pub name: String,
    pub description: String,
    pub vid: Option<u16>,
    pub pid: Option<u16>,
}

pub fn list_serial_ports() -> Vec<PortInfo> {
    match serialport::available_ports() {
        Ok(ports) => ports
            .into_iter()
            .map(|p| {
                let (description, vid, pid) = match &p.port_type {
                    serialport::SerialPortType::UsbPort(usb) => (
                        usb.product.clone().unwrap_or_default(),
                        Some(usb.vid),
                        Some(usb.pid),
                    ),
                    serialport::SerialPortType::BluetoothPort => {
                        ("Bluetooth".to_string(), None, None)
                    }
                    serialport::SerialPortType::PciPort => ("PCI".to_string(), None, None),
                    serialport::SerialPortType::Unknown => ("Unknown".to_string(), None, None),
                };
                PortInfo {
                    name: p.port_name,
                    description,
                    vid,
                    pid,
                }
            })
            .collect(),
        Err(e) => {
            log::error!("Failed to enumerate serial ports: {}", e);
            Vec::new()
        }
    }
}
