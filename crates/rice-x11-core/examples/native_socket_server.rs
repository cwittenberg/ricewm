use rice_x11_core::{DisplayConfig, DisplayServer};
use std::io::{Read, Write};
use std::os::unix::net::UnixListener;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args()
        .nth(1)
        .map_or_else(|| PathBuf::from("/tmp/.X11-unix/X88"), PathBuf::from);
    if path.exists() {
        return Err(format!("refusing to replace existing socket {}", path.display()).into());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let listener = UnixListener::bind(&path)?;
    let mut server = DisplayServer::new(DisplayConfig::default());
    let handle = server.open_client().map_err(|error| format!("{error:?}"))?;
    let (mut stream, _) = listener.accept()?;
    let mut input = vec![0_u8; 64 * 1024].into_boxed_slice();
    let mut output = vec![0_u8; 64 * 1024];
    loop {
        let count = stream.read(&mut input)?;
        if count == 0 {
            break;
        }
        server
            .feed(handle, &input[..count])
            .map_err(|error| format!("{error:?}"))?;
        loop {
            let count = server
                .drain(handle, &mut output)
                .map_err(|error| format!("{error:?}"))?;
            if count == 0 {
                break;
            }
            stream.write_all(&output[..count])?;
        }
    }
    drop(stream);
    drop(listener);
    std::fs::remove_file(path)?;
    Ok(())
}
