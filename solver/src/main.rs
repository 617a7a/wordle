use std::io::{Read, Write};

const WORDS: &str = include_str!("../../wordle/src/words.txt");

fn main() {
    // run the program as a child process, and capture the output and input
    let mut child = std::process::Command::new("cargo")
        .arg("run")
        .arg("--quiet")
        .arg("-p")
        .arg("wordle")
        .stdout(std::process::Stdio::piped())
        .stdin(std::process::Stdio::piped())
        .spawn()
        .unwrap();

    loop {
        // read the output from the child process
        let mut output = String::new();
        child.stdout.as_mut().unwrap().read_to_string(&mut output).unwrap();

        // if the output contains the word "I have a 5 letter word in mind", then we can start guessing
        if output.contains("I have a 5 letter word in mind") {
            // split the output into lines
            let lines = output.split('\n').collect::<Vec<&str>>();

            // the last line is the prompt
            let prompt = lines[lines.len() - 1];

            // the word is the last word in the prompt
            let word = prompt.split(' ').last().unwrap();

            // guess the word
            let guess = guess_word(word);

            // write the guess to the child process
            child.stdin.as_mut().unwrap().write_all(guess.as_bytes()).unwrap();
            child.stdin.as_mut().unwrap().write_all(b"\n").unwrap();
        }
    }
}

fn guess_word(green_letters: Vec<char>, yellow_letters: Vec<char>, red_letters: Vec<char>, word: &str) -> String {
    // if the word is already guessed, return it
    if green_letters.len() == word.len() {
        return word.to_string();
    }

    // if the word is not guessed, guess the first letter
    let first_letter = word.chars().nth(0).unwrap();

    // if the first letter is already guessed, guess the next letter
    if green_letters.contains(&first_letter) {
        return guess_word(green_letters, yellow_letters, red_letters, &word[1..]);
    }

    // if the first letter is not guessed, guess it
    return first_letter.to_string();
}

// is_yellow returns true if the letter uses ANSI yellow color codes
fn is_yellow(letter: &str) {
    
}
