use std::io::{self, Write};

pub fn prompt_confirm() -> bool {
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    let mut input = String::new();
    loop {
        print!("OK? [yN] ");
        stdout.flush().unwrap();
        stdin.read_line(&mut input).unwrap();
        {
            let input = input.trim_right();
            if input == "y" || input == "Y" {
                return true;
            } else if input == "n" || input == "N" || input == "" {
                return false;
            } else {
                println!("Invalid input: {:?}", input);
            }
        }
        input.clear();
    }
}
