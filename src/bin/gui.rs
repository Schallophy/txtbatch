#![windows_subsystem = "windows"]

fn main() -> Result<(), Box<dyn std::error::Error>> {
    txtbatch::launch_gui()?;
    Ok(())
}
