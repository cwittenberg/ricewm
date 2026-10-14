/// Atomically replace trusted native fullscreen outputs from at most sixteen
/// staged little-endian u16 (x, y, width, height) records. Zero clears ownership.
#[unsafe(no_mangle)]
pub extern "C" fn rice_wm_native_fullscreen_regions(count: u32) -> u32 {
    if count > 16 {
        return 0;
    }
    let Ok(buffer) = super::io_buffer().lock() else {
        return 0;
    };
    let Some(bytes) = buffer.get(..count as usize * 8) else {
        return 0;
    };
    let regions = bytes
        .chunks_exact(8)
        .map(|record| {
            (
                u16::from_le_bytes([record[0], record[1]]),
                u16::from_le_bytes([record[2], record[3]]),
                u16::from_le_bytes([record[4], record[5]]),
                u16::from_le_bytes([record[6], record[7]]),
            )
        })
        .collect();
    super::with_server(|server| u32::from(server.set_native_fullscreen_regions(regions)))
        .unwrap_or(0)
}
