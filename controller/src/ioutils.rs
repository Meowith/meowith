use std::error::Error;
use std::fs::File;
use std::io::Read;

pub fn read_file(path: &String) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut file = match File::open(path) {
        Ok(file) => file,
        Err(e) => return Err(Box::new(e)),
    };
    let mut buffer: Vec<u8> = Vec::new();

    file.read_to_end(&mut buffer)?;

    Ok(buffer)
}
