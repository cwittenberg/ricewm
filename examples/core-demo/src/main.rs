use std::{fs::{self, OpenOptions}, io::Write, path::PathBuf};
use rice_layout::{Rect, TileLayout};
use rice_files::GuestPath;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args_os().nth(1).map(PathBuf::from)
        .ok_or("usage: ricewm-core-demo <new-output-file>")?;
    let area = Rect { x: 0, y: 0, width: 1024, height: 768 };
    let mut layout = TileLayout::<u32>::default();
    layout.sync(&[1, 2], area);
    let positions = layout.rectangles(area);
    assert_eq!(positions.len(), 2);
    let serialized = layout.snapshot(|id| id);
    let restored = TileLayout::from_snapshot(&serialized, Some).ok_or("invalid arrangement")?;
    assert_eq!(restored.rectangles(area), positions);
    let result = rice_calc::evaluate("7 * 8", false).map_err(|e| format!("{e:?}"))?;
    let location = GuestPath::resolve(&GuestPath::root(), "Documents/./demo.txt")
        .map_err(|e| format!("{e:?}"))?;
    let text = format!("RiceWM core demo\nLocation: {}\n7 * 8 = {result}\nWindows: {positions:?}\n", location.as_str());
    let mut file = OpenOptions::new().write(true).create_new(true).open(&path)?;
    file.write_all(text.as_bytes())?;
    file.sync_all()?;
    drop(file);
    assert_eq!(fs::read_to_string(&path)?, text);
    println!("PASS: layout, restore, calculate, save and reopen: {}", path.display());
    Ok(())
}
