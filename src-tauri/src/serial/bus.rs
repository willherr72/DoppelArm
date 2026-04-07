use std::time::Duration;

use serde::Serialize;

/// Low-level servo bus communication for Feetech STS3215 servos.
/// Uses the Feetech serial protocol (Dynamixel Protocol v1 compatible).
///
/// The STS3215 uses half-duplex async serial with the packet format:
/// TX: [0xFF, 0xFF, ID, LENGTH, INSTRUCTION, PARAMS..., CHECKSUM]
/// RX: [0xFF, 0xFF, ID, LENGTH, ERROR, PARAMS..., CHECKSUM]
pub struct ServoBus {
    port: Box<dyn serialport::SerialPort>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ServoStatus {
    pub id: u8,
    pub position: i32,
    pub speed: i32,
    pub load: i32,
    pub voltage: u8,
    pub temperature: u8,
}

// Feetech STS3215 register addresses
const REG_ID: u8 = 5;
const REG_BAUD_RATE: u8 = 6;
const REG_MIN_ANGLE_LIMIT: u8 = 9;
const REG_MAX_ANGLE_LIMIT: u8 = 11;
const REG_TORQUE_ENABLE: u8 = 40;
const REG_GOAL_POSITION: u8 = 42;
const REG_GOAL_SPEED: u8 = 46;
const REG_LOCK: u8 = 48;
const REG_PRESENT_POSITION: u8 = 56;
const REG_PRESENT_SPEED: u8 = 58;
const REG_PRESENT_LOAD: u8 = 60;
const REG_PRESENT_VOLTAGE: u8 = 62;
const REG_PRESENT_TEMPERATURE: u8 = 63;
const REG_MODE: u8 = 33;

// Instruction set
const INST_PING: u8 = 0x01;
const INST_READ: u8 = 0x02;
const INST_WRITE: u8 = 0x03;
const INST_REG_WRITE: u8 = 0x04;
const INST_ACTION: u8 = 0x05;
const INST_SYNC_READ: u8 = 0x82;
const INST_SYNC_WRITE: u8 = 0x83;

// Broadcast ID
const BROADCAST_ID: u8 = 0xFE;

impl ServoBus {
    pub fn new(port_name: &str, baud_rate: u32) -> Result<Self, String> {
        let port = serialport::new(port_name, baud_rate)
            .timeout(Duration::from_millis(20))
            .data_bits(serialport::DataBits::Eight)
            .parity(serialport::Parity::None)
            .stop_bits(serialport::StopBits::One)
            .flow_control(serialport::FlowControl::None)
            .open()
            .map_err(|e| format!("Failed to open serial port {}: {}", port_name, e))?;

        Ok(Self { port })
    }

    /// Scan for servo IDs on the bus by pinging each ID in range.
    pub fn scan(&mut self, id_range: std::ops::RangeInclusive<u8>) -> Vec<u8> {
        let mut found = Vec::new();
        for id in id_range {
            if self.ping(id).is_ok() {
                found.push(id);
            }
        }
        found
    }

    /// Ping a servo to check if it's present on the bus.
    pub fn ping(&mut self, id: u8) -> Result<(), String> {
        self.flush_input();
        let packet = self.build_packet(id, INST_PING, &[]);
        self.write_packet(&packet)?;
        let _response = self.read_response(id)?;
        Ok(())
    }

    /// Read the current position of a single servo.
    pub fn read_position(&mut self, id: u8) -> Result<i32, String> {
        let data = self.read_register(id, REG_PRESENT_POSITION, 2)?;
        Ok(i16::from_le_bytes([data[0], data[1]]) as i32)
    }

    /// Read positions of multiple servos using sync read.
    pub fn sync_read_positions(&mut self, ids: &[u8]) -> Result<Vec<i32>, String> {
        self.sync_read(ids, REG_PRESENT_POSITION, 2)
            .map(|data| {
                data.chunks(2)
                    .map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]) as i32)
                    .collect()
            })
    }

    /// Write goal position to a single servo.
    pub fn write_position(&mut self, id: u8, position: i32) -> Result<(), String> {
        let pos = (position as i16).to_le_bytes();
        self.write_register(id, REG_GOAL_POSITION, &pos)
    }

    /// Write goal positions to multiple servos using sync write.
    pub fn sync_write_positions(&mut self, ids: &[u8], positions: &[i32]) -> Result<(), String> {
        if ids.len() != positions.len() {
            return Err("IDs and positions must have the same length".to_string());
        }
        let data: Vec<Vec<u8>> = positions
            .iter()
            .map(|&p| (p as i16).to_le_bytes().to_vec())
            .collect();
        self.sync_write(ids, REG_GOAL_POSITION, 2, &data)
    }

    /// Write goal speed for a single servo.
    pub fn write_speed(&mut self, id: u8, speed: i32) -> Result<(), String> {
        let spd = (speed as i16).to_le_bytes();
        self.write_register(id, REG_GOAL_SPEED, &spd)
    }

    /// Enable or disable torque on a single servo.
    pub fn set_torque_enable(&mut self, id: u8, enable: bool) -> Result<(), String> {
        self.write_register(id, REG_TORQUE_ENABLE, &[if enable { 1 } else { 0 }])
    }

    /// Enable or disable torque on multiple servos.
    pub fn sync_set_torque(&mut self, ids: &[u8], enable: bool) -> Result<(), String> {
        let val = if enable { 1u8 } else { 0u8 };
        let data: Vec<Vec<u8>> = ids.iter().map(|_| vec![val]).collect();
        self.sync_write(ids, REG_TORQUE_ENABLE, 1, &data)
    }

    /// Read the operating mode of a servo.
    pub fn read_mode(&mut self, id: u8) -> Result<u8, String> {
        let data = self.read_register(id, REG_MODE, 1)?;
        Ok(data[0])
    }

    /// Set the operating mode (0=position servo, 1=speed closed-loop, 3=step).
    pub fn write_mode(&mut self, id: u8, mode: u8) -> Result<(), String> {
        self.write_register(id, REG_MODE, &[mode])
    }

    /// Read full status of a servo.
    pub fn read_status(&mut self, id: u8) -> Result<ServoStatus, String> {
        // Read a block from present_position through temperature (8 bytes)
        let data = self.read_register(id, REG_PRESENT_POSITION, 8)?;
        Ok(ServoStatus {
            id,
            position: i16::from_le_bytes([data[0], data[1]]) as i32,
            speed: i16::from_le_bytes([data[2], data[3]]) as i32,
            load: i16::from_le_bytes([data[4], data[5]]) as i32,
            voltage: data[6],
            temperature: data[7],
        })
    }

    /// Change the ID of a servo (requires the servo to be the only one on the bus, or addressed by current ID).
    pub fn set_servo_id(&mut self, current_id: u8, new_id: u8) -> Result<(), String> {
        // Unlock EEPROM
        self.write_register(current_id, REG_LOCK, &[0])?;
        // Write new ID
        self.write_register(current_id, REG_ID, &[new_id])?;
        Ok(())
    }

    /// Set baud rate of a servo. Common values: 0=1M, 1=500K, 2=250K, 3=128K, 4=115200, 7=38400
    pub fn set_baud_rate(&mut self, id: u8, baud_index: u8) -> Result<(), String> {
        self.write_register(id, REG_LOCK, &[0])?;
        self.write_register(id, REG_BAUD_RATE, &[baud_index])
    }

    /// Set angle limits for a servo.
    pub fn set_angle_limits(
        &mut self,
        id: u8,
        min: i32,
        max: i32,
    ) -> Result<(), String> {
        self.write_register(id, REG_LOCK, &[0])?;
        let min_bytes = (min as i16).to_le_bytes();
        let max_bytes = (max as i16).to_le_bytes();
        self.write_register(id, REG_MIN_ANGLE_LIMIT, &min_bytes)?;
        self.write_register(id, REG_MAX_ANGLE_LIMIT, &max_bytes)
    }

    /// Write a value to the servo's center offset register (address 40 -> write 128 for calibration).
    pub fn write_offset_calibration(&mut self, id: u8) -> Result<(), String> {
        // STS3215: writing 128 to address 40 sets current position as center
        self.write_register(id, 40, &[128])
    }

    // ---- Low-level protocol implementation ----

    fn build_packet(&self, id: u8, instruction: u8, params: &[u8]) -> Vec<u8> {
        let length = (params.len() + 2) as u8; // params + instruction + checksum
        let mut packet = vec![0xFF, 0xFF, id, length, instruction];
        packet.extend_from_slice(params);
        let checksum = self.calculate_checksum(&packet[2..]);
        packet.push(checksum);
        packet
    }

    fn calculate_checksum(&self, data: &[u8]) -> u8 {
        let sum: u8 = data.iter().fold(0u8, |acc, &b| acc.wrapping_add(b));
        !sum
    }

    fn write_packet(&mut self, packet: &[u8]) -> Result<(), String> {
        use std::io::Write;
        self.port
            .write_all(packet)
            .map_err(|e| format!("Write failed: {}", e))?;
        self.port
            .flush()
            .map_err(|e| format!("Flush failed: {}", e))?;
        Ok(())
    }

    fn read_response(&mut self, expected_id: u8) -> Result<Vec<u8>, String> {
        use std::io::Read;

        // Read header: 0xFF 0xFF ID LENGTH
        let mut header = [0u8; 4];
        self.port
            .read_exact(&mut header)
            .map_err(|e| format!("Read header failed (id={}): {}", expected_id, e))?;

        if header[0] != 0xFF || header[1] != 0xFF {
            return Err(format!("Invalid header: {:02X} {:02X}", header[0], header[1]));
        }

        if header[2] != expected_id {
            return Err(format!(
                "ID mismatch: expected {}, got {}",
                expected_id, header[2]
            ));
        }

        let length = header[3] as usize;
        if length < 2 || length > 254 {
            return Err(format!("Invalid length: {}", length));
        }

        // Read remaining bytes: ERROR + PARAMS + CHECKSUM
        let mut body = vec![0u8; length];
        self.port
            .read_exact(&mut body)
            .map_err(|e| format!("Read body failed: {}", e))?;

        let error = body[0];
        if error != 0 {
            return Err(format!("Servo error (id={}): 0x{:02X}", expected_id, error));
        }

        // Verify checksum
        let mut check_data = vec![header[2], header[3]];
        check_data.extend_from_slice(&body[..body.len() - 1]);
        let expected_checksum = self.calculate_checksum(&check_data);
        let actual_checksum = body[body.len() - 1];
        if expected_checksum != actual_checksum {
            return Err(format!(
                "Checksum mismatch: expected 0x{:02X}, got 0x{:02X}",
                expected_checksum, actual_checksum
            ));
        }

        // Return params (skip error byte, skip checksum)
        Ok(body[1..body.len() - 1].to_vec())
    }

    fn read_register(&mut self, id: u8, address: u8, length: u8) -> Result<Vec<u8>, String> {
        self.flush_input();
        let packet = self.build_packet(id, INST_READ, &[address, length]);
        self.write_packet(&packet)?;
        self.read_response(id)
    }

    fn write_register(&mut self, id: u8, address: u8, data: &[u8]) -> Result<(), String> {
        self.flush_input();
        let mut params = vec![address];
        params.extend_from_slice(data);
        let packet = self.build_packet(id, INST_WRITE, &params);
        self.write_packet(&packet)?;
        // Write instruction returns a status packet (unless broadcast)
        if id != BROADCAST_ID {
            let _response = self.read_response(id)?;
        }
        Ok(())
    }

    fn sync_read(&mut self, ids: &[u8], address: u8, length: u8) -> Result<Vec<u8>, String> {
        self.flush_input();
        // Sync read packet: [address, length, id1, id2, ...]
        let mut params = vec![address, length];
        params.extend_from_slice(ids);
        let packet = self.build_packet(BROADCAST_ID, INST_SYNC_READ, &params);
        self.write_packet(&packet)?;

        // Each servo responds individually
        let mut all_data = Vec::new();
        for &id in ids {
            let data = self.read_response(id)?;
            all_data.extend_from_slice(&data);
        }
        Ok(all_data)
    }

    fn sync_write(
        &mut self,
        ids: &[u8],
        address: u8,
        data_length: u8,
        data: &[Vec<u8>],
    ) -> Result<(), String> {
        self.flush_input();
        // Sync write packet: [address, data_length, id1, data1..., id2, data2..., ...]
        let mut params = vec![address, data_length];
        for (i, id) in ids.iter().enumerate() {
            params.push(*id);
            params.extend_from_slice(&data[i]);
        }
        let packet = self.build_packet(BROADCAST_ID, INST_SYNC_WRITE, &params);
        self.write_packet(&packet)?;
        // Sync write has no response
        Ok(())
    }

    fn flush_input(&mut self) {
        use std::io::Read;
        let mut buf = [0u8; 256];
        // Drain any pending data
        while self.port.read(&mut buf).unwrap_or(0) > 0 {}
    }
}
