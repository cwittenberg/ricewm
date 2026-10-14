// Original RiceWM fallback artwork, MIT licensed. ARGB pixels.
// The compositor can also accept caller-supplied application icons.
const fn icon(kind: u32) -> [u32; 256] {
    let mut pixels = [0; 256];
    let mut y = 0;
    while y < 16 {
        let mut x = 0;
        while x < 16 {
            let inside = x >= 2 && x < 14 && y >= 3 && y < 13;
            let border = x == 2 || x == 13 || y == 3 || y == 12;
            let detail = y == 6 + kind as usize % 3 && x >= 5 && x < 11;
            pixels[y * 16 + x] = if !inside { 0 } else if border { 0xff324861 }
                else if detail { 0xffffffff } else { 0xff639cdb + (kind % 4) * 0x000b0700 };
            x += 1;
        }
        y += 1;
    }
    pixels
}
pub static DESKTOP: [u32; 256] = icon(0);
pub static FOLDER_OPEN: [u32; 256] = icon(1);
pub static MSDOS: [u32; 256] = icon(2);
pub static NOTEPAD: [u32; 256] = icon(3);
pub static CALCULATOR: [u32; 256] = icon(4);
pub static TASK: [u32; 256] = icon(5);
pub static PAINT: [u32; 256] = icon(6);
pub static MEDIA_PLAYER: [u32; 256] = icon(7);
pub static SETTINGS: [u32; 256] = icon(8);
pub static WINDOWS: [u32; 256] = icon(9);
pub static INTERNET_EXPLORER: [u32; 256] = icon(10);
pub static MAIL: [u32; 256] = icon(11);
