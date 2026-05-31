use serde::{Deserialize, Serialize};
use std::io::{self, Read, Write};

const MAX_FRAME_SIZE: u32 = 1024 * 1024; // 1 MiB

#[derive(Serialize, Deserialize, Debug)]
pub enum InputMsg {
    MouseMove { x: f64, y: f64 },
    MouseButton { button: MouseButton, pressed: bool },
    Wheel { dx: i64, dy: i64 },
}

#[derive(Serialize, Deserialize, Debug)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

/// Write a length-prefixed bincode frame to the writer.
pub fn write_msg<W: Write>(writer: &mut W, msg: &InputMsg) -> io::Result<()> {
    let payload = bincode::serialize(msg)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    let len = payload.len() as u32;
    writer.write_all(&len.to_le_bytes())?;
    writer.write_all(&payload)?;
    writer.flush()?;
    Ok(())
}

/// Read a length-prefixed bincode frame from the reader.
pub fn read_msg<R: Read>(reader: &mut R) -> io::Result<InputMsg> {
    let mut len_buf = [0u8; 4];
    reader.read_exact(&mut len_buf)?;
    let len = u32::from_le_bytes(len_buf);

    if len > MAX_FRAME_SIZE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("frame too large: {} bytes (max {})", len, MAX_FRAME_SIZE),
        ));
    }

    let mut payload = vec![0u8; len as usize];
    reader.read_exact(&mut payload)?;

    bincode::deserialize(&payload)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}
