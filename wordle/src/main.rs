use std::io::Write;

use colored::Colorize;

const WORDS: &str = include_str!("words.txt");

// A CLI version of Wordle
fn main() {
    let words: Vec<&str> = WORDS.split_whitespace().collect();
    let word = words[rand::random::<usize>() % words.len()];

    println!("I have a 5 letter word in mind. Can you guess it?");
    let mut chances_left = 5;

    if std::env::var("DEBUG").is_ok() {
        println!("(debug: {})", word.blue());
    }

    // loop until the user guesses the word or runs out of chances
    loop {
        match process_input(word, read_line()) {
            Ok(win) => {
                if win {
                    println!("You guessed it right!");
                    break;
                } else {
                    chances_left -= 1;
                    if chances_left == 0 {
                        println!("You ran out of chances. The word was {}!", word.blue());
                        break;
                    } else {
                        println!(
                            "{} You have {} chances left.",
                            "You guessed it wrong.".red(),
                            chances_left
                        );
                    }
                }
            }
            Err(ProcessInputError::InvalidLength) => {
                println!("Please enter a word of length {}", word.len())
            }
        }
    }
}

enum ProcessInputError {
    InvalidLength,
}

/// Checks the word against the input and returns true if the word is guessed correctly
/// We also print the word, with some formatting
fn process_input(word: &str, input: String) -> Result<bool, ProcessInputError> {
    if input == "exit" {
        println!("Exiting. The word was {}!", word.blue());
        std::process::exit(0);
    }
    if input.len() != word.len() {
        return Err(ProcessInputError::InvalidLength);
    }
    let mut guessed = String::new();
    let mut correct = 0;
    for (i, c) in input.chars().enumerate() {
        // right letter, right position
        if word.contains(c) {
            if word.chars().nth(i).unwrap() == c {
                guessed.push_str(&format!("{} ", c.to_string().green()));
                correct += 1;
            } else {
                // right letter, wrong position
                guessed.push_str(&format!("{} ", c.to_string().yellow()));
            }
        // wrong letter
        } else {
            guessed.push_str(&format!(
                "{} ",
                input.chars().nth(i).unwrap().to_string().red()
            ));
        }
    }

    println!("\n{}", guessed);
    Ok(correct == word.len())
}

/// Reads a line from stdin and returns it as a String
fn read_line() -> String {
    print!(">> ");
    std::io::stdout().flush().unwrap();
    let mut input = String::new();
    std::io::stdin().read_line(&mut input).unwrap();
    input.trim().to_string().to_lowercase()
}
