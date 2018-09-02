use std::io::{self, Result, Write};

const PROMPT_YES: [&str; 2] = ["y", "Y"];
const PROMPT_NO: [&str; 3] = ["n", "N", ""];

pub fn prompt_confirm() -> Result<bool> {
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    let mut input = String::new();
    loop {
        print!("OK? [yN] ");
        stdout.flush()?;
        stdin.read_line(&mut input)?;
        {
            let input = input.trim_right();
            if PROMPT_YES.contains(&input) {
                return Ok(true);
            } else if PROMPT_NO.contains(&input) {
                return Ok(false);
            } else {
                println!("Invalid input: {:?}", input);
            }
        }
        input.clear();
    }
}
