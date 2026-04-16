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
const REG_LOCK: u8 = 55;
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
            .timeout(Duration::from_millis(30))
            .data_bits(serialport::DataBits::Eight)
            .parity(serialport::Parity::None)
            .stop_bits(serialport::StopBits::One)
            .flow_control(serialport::FlowControl::None)
            .open()
            .map_err(|e| format!("Failed to open serial port {}: {}", port_name, e))?;

        Ok(Self { port })
    }

    /// Scan for servo IDs on the bus by pinging each ID in range.
    /// Uses a fast no-retry ping for efficiency.
    pub fn scan(&mut self, id_range: std::ops::RangeInclusive<u8>) -> Vec<u8> {
        let mut found = Vec::new();
        for id in id_range {
            if self.ping_fast(id).is_ok() {
                found.push(id);
            }
        }
        found
    }

    /// Ping a servo to check if it's present on the bus (with retry).
    pub fn ping(&mut self, id: u8) -> Result<(), String> {
        self.flush_input();
        let packet = self.build_packet(id, INST_PING, &[]);
        let _response = self.send_and_receive(&packet, id)?;
        Ok(())
    }

    /// Fast ping without retries — used by scan to quickly probe many IDs.
    pub fn ping_fast(&mut self, id: u8) -> Result<(), String> {
        use std::io::{Read, Write};
        use serialport::ClearBuffer;

        let _ = self.port.clear(ClearBuffer::Input);

        let packet = self.build_packet(id, INST_PING, &[]);
        self.port.write_all(&packet).map_err(|e| e.to_string())?;
        self.port.flush().map_err(|e| e.to_string())?;

        // Quick read with one timeout cycle
        let mut buf = vec![0u8; 32];
        let mut total = 0;
        let deadline = std::time::Instant::now() + std::time::Duration::from_millis(40);

        while std::time::Instant::now() < deadline && total < buf.len() {
            match self.port.read(&mut buf[total..]) {
                Ok(0) => break,
                Ok(n) => {
                    total += n;
                    if let Some(result) = self.try_parse_response(&buf[..total], id) {
                        return result.map(|_| ());
                    }
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => {
                    if total > 0 {
                        break;
                    }
                }
                Err(e) => return Err(e.to_string()),
            }
        }

        if total > 0 {
            self.try_parse_response(&buf[..total], id)
                .map(|r| r.map(|_| ()))
                .unwrap_or_else(|| Err("no valid response".to_string()))
        } else {
            Err("no response".to_string())
        }
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

    /// Change the ID of a servo.
    ///
    /// EEPROM write sequence for Feetech STS3215:
    ///   1. Unlock EEPROM (write 0 to REG_LOCK)
    ///   2. Write new ID
    ///   3. Re-lock EEPROM (write 1 to REG_LOCK) — this commits to flash
    ///
    /// Without step 3, the write only goes to RAM mirror and is lost on power cycle.
    pub fn set_servo_id(&mut self, current_id: u8, new_id: u8) -> Result<(), String> {
        if current_id == new_id {
            return self.ping(current_id);
        }

        // Step 1: Unlock EEPROM via current ID
        self.write_register(current_id, REG_LOCK, &[0])?;

        // Step 2: Write the new ID. The response will come back with NEW id,
        // so we send the packet but don't try to parse the status response.
        let packet = self.build_packet(current_id, INST_WRITE, &[REG_ID, new_id]);
        self.flush_input();
        self.write_packet(&packet)?;
        std::thread::sleep(std::time::Duration::from_millis(20));
        let _ = self.port.clear(serialport::ClearBuffer::Input);

        // Step 3: Re-lock EEPROM using the NEW id to commit the write to flash.
        // Without this, the ID change is lost on power cycle!
        std::thread::sleep(std::time::Duration::from_millis(10));
        self.write_register(new_id, REG_LOCK, &[1])
            .map_err(|e| format!("EEPROM commit (relock) failed: {}", e))?;

        // Verify the motor now responds at the new ID
        self.ping(new_id)
            .map_err(|e| format!("ID change verification failed: {}", e))?;

        Ok(())
    }

    /// Set baud rate of a servo. Common values: 0=1M, 1=500K, 2=250K, 3=128K, 4=115200, 7=38400
    ///
    /// Uses the same unlock -> write -> relock sequence as set_servo_id.
    /// After this call, the servo will respond at the NEW baud rate.
    pub fn set_baud_rate(&mut self, id: u8, baud_index: u8) -> Result<(), String> {
        // Unlock EEPROM
        self.write_register(id, REG_LOCK, &[0])?;

        // Write the new baud rate. After this, the servo switches to the new
        // rate so we can't get a valid status response back at the current rate.
        let packet = self.build_packet(id, INST_WRITE, &[REG_BAUD_RATE, baud_index]);
        self.flush_input();
        self.write_packet(&packet)?;
        std::thread::sleep(std::time::Duration::from_millis(20));
        let _ = self.port.clear(serialport::ClearBuffer::Input);

        // Note: we can't re-lock here because the servo is now at a different
        // baud rate. The caller should re-open the bus at the new rate and
        // then re-lock to commit. For our use case (always writing baud_index=0
        // which is already the default 1Mbps), this is typically a no-op and
        // we skip the relock.
        Ok(())
    }

    /// Set angle limits for a servo. Uses unlock -> write -> relock sequence
    /// to commit values to EEPROM.
    pub fn set_angle_limits(
        &mut self,
        id: u8,
        min: i32,
        max: i32,
    ) -> Result<(), String> {
        // Unlock EEPROM
        self.write_register(id, REG_LOCK, &[0])?;
        let min_bytes = (min as i16).to_le_bytes();
        let max_bytes = (max as i16).to_le_bytes();
        self.write_register(id, REG_MIN_ANGLE_LIMIT, &min_bytes)?;
        self.write_register(id, REG_MAX_ANGLE_LIMIT, &max_bytes)?;
        // Re-lock to commit
        self.write_register(id, REG_LOCK, &[1])
    }

    /// Enable multi-turn / continuous rotation mode for a servo.
    /// Sets both min and max angle limits to 0, which puts the STS3215
    /// into wheel mode where it can rotate continuously without wrapping.
    pub fn enable_continuous_rotation(&mut self, id: u8) -> Result<(), String> {
        self.set_angle_limits(id, 0, 0)
    }

    /// Restore the default single-turn 0-4095 position range for a servo.
    pub fn enable_single_turn(&mut self, id: u8) -> Result<(), String> {
        self.set_angle_limits(id, 0, 4095)
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

        // Retry the entire request a few times to handle transient bus errors.
        let mut last_buf: Vec<u8> = Vec::new();
        for retry in 0..3 {
            // Clear any stale input data
            let _ = self.port.clear(ClearBuffer::Input);

            self.port
                .write_all(packet)
                .map_err(|e| format!("Write failed: {}", e))?;
            self.port
                .flush()
                .map_err(|e| format!("Flush failed: {}", e))?;

            // Read response bytes. Keep reading until we get a parseable
            // response or hit the overall deadline.
            let mut buf = vec![0u8; 128];
            let mut total_read = 0;
            let deadline = std::time::Instant::now() + std::time::Duration::from_millis(100);

            while total_read < buf.len() && std::time::Instant::now() < deadline {
                match self.port.read(&mut buf[total_read..]) {
                    Ok(0) => break,
                    Ok(n) => {
                        total_read += n;
                        if let Some(result) = self.try_parse_response(&buf[..total_read], expected_id) {
                            return result;
                        }
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => {
                        // Don't give up — keep trying until deadline
                        if total_read > 0 {
                            // Have some bytes but read timed out waiting for more.
                            // Try once more then break if still no progress.
                            std::thread::sleep(std::time::Duration::from_millis(2));
                            continue;
                        }
                    }
                    Err(e) => return Err(format!("Read failed: {}", e)),
                }
            }

            last_buf = buf[..total_read].to_vec();

            // If we got nothing, retry from scratch
            if total_read == 0 {
                std::thread::sleep(std::time::Duration::from_millis(5));
                continue;
            }

            // Got some data but couldn't parse it — retry once more
            if retry < 2 {
                std::thread::sleep(std::time::Duration::from_millis(5));
                continue;
            }
        }

        // All retries exhausted
        let total_read = last_buf.len();
        let buf = last_buf;

        // Final attempt to parse everything we collected
        if total_read > 0 {
            if let Some(result) = self.try_parse_response(&buf[..total_read], expected_id) {
                return result;
            }
            let rx_hex: String = buf[..total_read].iter().map(|b| format!("{:02X}", b)).collect::<Vec<_>>().join(" ");
            let tx_hex: String = packet.iter().map(|b| format!("{:02X}", b)).collect::<Vec<_>>().join(" ");
            Err(format!("id={} TX[{}] RX[{}]", expected_id, tx_hex, rx_hex))
        } else {
            let tx_hex: String = packet.iter().map(|b| format!("{:02X}", b)).collect::<Vec<_>>().join(" ");
            Err(format!("id={} no response TX[{}]", expected_id, tx_hex))
        }
    }

    /// Try to find and parse a valid response packet for the expected ID
    /// within a byte buffer. Scans for FF FF [id] [len] patterns.
    fn try_parse_response(&self, buf: &[u8], expected_id: u8) -> Option<Result<Vec<u8>, String>> {
        // Try to parse a response packet starting at the given offset.
        // Returns Some(Ok(params)) on valid packet, Some(Err) on servo error,
        // None if no valid packet found.
        let try_at = |id_pos: usize| -> Option<Result<Vec<u8>, String>> {
            // id_pos = position of the ID byte in the buffer
            if id_pos + 2 >= buf.len() {
                return None;
            }

            let resp_id = buf[id_pos];
            let length = buf[id_pos + 1] as usize;

            if resp_id != expected_id {
                return None;
            }

            if length < 2 || length > 64 {
                return None;
            }

            let packet_end = id_pos + 2 + length;
            if packet_end > buf.len() {
                return None;
            }

            let body = &buf[id_pos + 2..packet_end];

            // Verify checksum
            let mut check_data = vec![resp_id, length as u8];
            check_data.extend_from_slice(&body[..body.len() - 1]);
            let expected_checksum = self.calculate_checksum(&check_data);
            let actual_checksum = body[body.len() - 1];

            if expected_checksum != actual_checksum {
                return None;
            }

            let error = body[0];
            if error != 0 {
                return Some(Err(format!("Servo error (id={}): 0x{:02X}", expected_id, error)));
            }

            // Valid response! Return params (skip error byte, skip checksum)
            Some(Ok(body[1..body.len() - 1].to_vec()))
        };

        // Strategy: scan for any valid packet by trying every position.
        // Accepts FF FF [id], FF [id] (single FF header lost), or [id] directly
        // (both FF bytes lost). Validation is done via checksum.
        for i in 0..buf.len() {
            // Try after FF FF (canonical)
            if i + 1 < buf.len() && buf[i] == 0xFF && buf[i + 1] == 0xFF {
                if let Some(result) = try_at(i + 2) {
                    return Some(result);
                }
            }
            // Try after single FF (first FF lost on direction switch)
            if buf[i] == 0xFF {
                if let Some(result) = try_at(i + 1) {
                    return Some(result);
                }
            }
            // Try directly at this position (both FFs lost)
            if let Some(result) = try_at(i) {
                return Some(result);
            }
        }
        None
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
    }
}
