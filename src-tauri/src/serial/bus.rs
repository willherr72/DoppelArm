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
        let _response = self.send_and_receive(&packet, id)?;
        Ok(())
    }

    /// Read the current position of a single servo.
    pub fn read_position(&mut self, id: u8) -> Result<i32, String> {
        let data = self.read_register(id, REG_PRESENT_POSITION, 2)?;
        Ok(i16::from_le_bytes([data[0], data[1]]) as i32)
    }

    /// Read positions of multiple servos.
    /// Uses individual reads for maximum compatibility with STS3215 firmware.
    pub fn sync_read_positions(&mut self, ids: &[u8]) -> Result<Vec<i32>, String> {
        let mut positions = Vec::with_capacity(ids.len());
        for &id in ids {
            let pos = self.read_position(id)?;
            positions.push(pos);
        }
        Ok(positions)
    }

    /// Write goal position to a single servo.
    pub fn write_position(&mut self, id: u8, position: i32) -> Result<(), String> {
        let pos = (position as i16).to_le_bytes();
        self.write_register(id, REG_GOAL_POSITION, &pos)
    }

    /// Write goal positions to multiple servos using sync write.
    /// Sync write (0x83) is broadcast with no response, so it works on half-duplex.
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

    /// Write goal positions using individual writes (fallback).
    pub fn write_positions_individual(&mut self, ids: &[u8], positions: &[i32]) -> Result<(), String> {
        if ids.len() != positions.len() {
            return Err("IDs and positions must have the same length".to_string());
        }
        for (i, &id) in ids.iter().enumerate() {
            self.write_position(id, positions[i])?;
        }
        Ok(())
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
        for &id in ids {
            self.set_torque_enable(id, enable)?;
        }
        Ok(())
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

    /// Send a packet and read the response in one operation.
    /// Handles half-duplex echo by reading all available bytes and
    /// finding the valid response packet within them.
    fn send_and_receive(&mut self, packet: &[u8], expected_id: u8) -> Result<Vec<u8>, String> {
        use std::io::{Read, Write};
        use serialport::ClearBuffer;

        // Clear any stale input data
        let _ = self.port.clear(ClearBuffer::Input);

        self.port
            .write_all(packet)
            .map_err(|e| format!("Write failed: {}", e))?;
        self.port
            .flush()
            .map_err(|e| format!("Flush failed: {}", e))?;

        // Wait for servo to process command and respond.
        // STS3215 return delay is typically 0.5-1ms.
        std::thread::sleep(std::time::Duration::from_millis(8));

        // Read all available bytes
        let mut buf = vec![0u8; 128];
        let mut total_read = 0;

        // Read with multiple attempts to collect all bytes
        for _ in 0..3 {
            match self.port.read(&mut buf[total_read..]) {
                Ok(0) => break,
                Ok(n) => total_read += n,
                Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => break,
                Err(e) => return Err(format!("Read failed: {}", e)),
            }
            if total_read >= 6 {
                if let Some(result) = self.try_parse_response(&buf[..total_read], expected_id) {
                    return result;
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(3));
        }

        // Final attempt to parse everything we collected
        if total_read > 0 {
            if let Some(result) = self.try_parse_response(&buf[..total_read], expected_id) {
                return result;
            }
            let hex: Vec<String> = buf[..total_read].iter().map(|b| format!("{:02X}", b)).collect();
            Err(format!("No valid response for id={} in {} bytes: {}", expected_id, total_read, hex.join(" ")))
        } else {
            Err(format!("No response from servo id={}", expected_id))
        }
    }

    /// Try to find and parse a valid response packet for the expected ID
    /// within a byte buffer. Scans for FF FF [id] [len] patterns.
    fn try_parse_response(&self, buf: &[u8], expected_id: u8) -> Option<Result<Vec<u8>, String>> {
        for i in 0..buf.len().saturating_sub(5) {
            // Look for FF FF header
            if buf[i] != 0xFF || buf.get(i + 1) != Some(&0xFF) {
                continue;
            }

            let resp_id = buf[i + 2];
            let length = buf[i + 3] as usize;

            // Must match our expected ID
            if resp_id != expected_id {
                continue;
            }

            // Sanity check length
            if length < 2 || length > 64 {
                continue;
            }

            // Check if we have enough bytes for the full packet
            let packet_end = i + 4 + length;
            if packet_end > buf.len() {
                return None; // Need more bytes
            }

            let body = &buf[i + 4..packet_end];
            let error = body[0];

            // Verify checksum
            let mut check_data = vec![resp_id, length as u8];
            check_data.extend_from_slice(&body[..body.len() - 1]);
            let expected_checksum = self.calculate_checksum(&check_data);
            let actual_checksum = body[body.len() - 1];

            if expected_checksum != actual_checksum {
                continue; // Bad checksum, try next FF FF
            }

            if error != 0 {
                return Some(Err(format!("Servo error (id={}): 0x{:02X}", expected_id, error)));
            }

            // Valid response! Return params (skip error byte, skip checksum)
            return Some(Ok(body[1..body.len() - 1].to_vec()));
        }
        None // No valid response found yet
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

    fn read_register(&mut self, id: u8, address: u8, length: u8) -> Result<Vec<u8>, String> {
        self.flush_input();
        let packet = self.build_packet(id, INST_READ, &[address, length]);
        self.send_and_receive(&packet, id)
    }

    fn write_register(&mut self, id: u8, address: u8, data: &[u8]) -> Result<(), String> {
        self.flush_input();
        let mut params = vec![address];
        params.extend_from_slice(data);
        let packet = self.build_packet(id, INST_WRITE, &params);
        if id != BROADCAST_ID {
            let _response = self.send_and_receive(&packet, id)?;
        } else {
            self.write_packet(&packet)?;
        }
        Ok(())
    }

    fn sync_read(&mut self, ids: &[u8], address: u8, length: u8) -> Result<Vec<u8>, String> {
        // Use individual reads for compatibility
        let mut all_data = Vec::new();
        for &id in ids {
            let data = self.read_register(id, address, length)?;
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
        use serialport::ClearBuffer;
        let _ = self.port.clear(ClearBuffer::Input);
        // Small delay to let any in-flight bytes arrive before clearing
        std::thread::sleep(std::time::Duration::from_millis(1));
        let _ = self.port.clear(ClearBuffer::Input);
    }
}
