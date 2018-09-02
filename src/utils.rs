use std::io::{self, Write};

pub fn prompt_confirm(message: &str, default: bool) -> io::Result<bool> {
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    let mut input = String::new();
    loop {
        print!("{} [{}] ", message, if default { "Yn" } else { "yN" });
        stdout.flush()?;
        stdin.read_line(&mut input)?;
        {
            let input = input.trim_right();
            match input {
                "" => return Ok(default),
                "y" | "Y" => return Ok(true),
                "n" | "N" => return Ok(false),
                _ => eprintln!("Invalid input: {}", input),
            }
        }
        input.clear();
    }
}
