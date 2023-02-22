use colored::Colorize;
use rayon::prelude::*;
use smooth::Smooth;
use std::{collections::HashMap, io::Write, ops::AddAssign, sync::Mutex};

const WORDS: &str = include_str!("../../wordle/src/words.txt");

type GuessResult = [Character; 5];

fn main() {
    let mut words: Vec<String> = WORDS.split_whitespace().map(|s| s.to_string()).collect();
    let (strategy, first_guess) = choose_optimal_strategy(&words);
    println!("\nUsing strategy: {:?}", strategy);

    let mut known_info: Vec<GuessResult> = vec![];

    for i in 0..5 {
        if i == 0 {
            // for our first guess, we have no information, so we just guess the word
            // not as an actual word, but as the top 5 letters in the word list by frequency
            println!("First guess is {}!", first_guess.blue());
        } else {
            println!(
                "\nPlease enter the result of your guess ({} chances left)",
                5 - i
            );
            // after the first guess, we get input from the user which we can use to refine
            // our guess
            let guess_result = get_guess_result();
            known_info.push(guess_result);
            println!("Guess result: {:?}", guess_result);
            let start = std::time::Instant::now();
            let filtered_results = filter_using_known_info(&words, &known_info);
            println!(
                "Found {} possible words in {:?}",
                filtered_results.len(),
                start.elapsed()
            );
            words = filtered_results;
            if words.len() < 5 {
                let fmttd_list = words
                    .iter()
                    .map(|s| s.blue().to_string())
                    .collect::<Vec<String>>()
                    .join(", ");
                println!("Try one of these: {}", fmttd_list);
            } else {
                println!("Try {}", words[0].blue());
            }
        }
    }
}

#[derive(Clone, Copy)]
enum Character {
    /// The character is in the word, but not in the correct position
    Yellow(char),
    /// The character is in the word, and in the correct position
    Green(char),
    /// The character is not in the word
    Red(char),
    /// Used only as a placeholder during user input
    Empty,
}

impl std::fmt::Debug for Character {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Character::Yellow(c) => write!(f, "{}", c.to_string().yellow()),
            Character::Green(c) => write!(f, "{}", c.to_string().green()),
            Character::Red(c) => write!(f, "{}", c.to_string().red()),
            Character::Empty => write!(f, "{}", "-".blue()),
        }
    }
}

/// Filters a wordlist based on previous guess results
fn filter_using_known_info(words: &Vec<String>, known_info: &Vec<GuessResult>) -> Vec<String> {
    // we have a list of words, and we know some information about the word we're
    // looking for we process the words finding possible words that match
    // **all** the known information
    words
        .iter()
        .filter(|word| {
            known_info.iter().all(|guess| {
                guess.iter().enumerate().all(|(i, c)| match c {
                    // word contains all yellow characters
                    Character::Yellow(t) => word.contains(*t),
                    // word contains all green characters in the correct position
                    Character::Green(t) => word.chars().nth(i).unwrap() == *t,
                    // word doesn't contain any red characters
                    Character::Red(t) => !word.contains(*t),
                    Character::Empty => unreachable!("Empty character in known_info"),
                })
            })
        })
        .map(|word| word.to_string())
        .collect()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
enum FirstGuessStrategy {
    Simple,
    PositionAware,
}

/// Returns the optimal starting guess for the wordset
fn get_first_guess(words: &Vec<String>, strategy: FirstGuessStrategy) -> String {
    // position awareness on this wordset reduces possible solutions in 5 guesses by
    // ~3%
    let total_chars = words.iter().map(|word| word.len()).sum::<usize>();
    match strategy {
        FirstGuessStrategy::PositionAware => {
            let start = std::time::Instant::now();
            // our first guess is constructed off the most common character in each position
            let frequencies: [HashMap<char, usize>; 5] = words.iter().fold(
                [
                    HashMap::new(),
                    HashMap::new(),
                    HashMap::new(),
                    HashMap::new(),
                    HashMap::new(),
                ],
                |mut acc, word| {
                    for (i, c) in word.chars().enumerate() {
                        let count = acc[i].entry(c).or_insert(0);
                        *count += 1;
                    }
                    acc
                },
            );

            println!(
                "Counted {} characters in {:?}",
                total_chars,
                start.elapsed()
            );

            let start = std::time::Instant::now();
            // find the most likely character in each position
            let guess = frequencies.iter().fold(String::new(), |mut acc, freq| {
                let (c, _) = freq.iter().max_by_key(|(_, count)| *count).unwrap();
                acc.push(*c);
                acc
            });

            println!(
                "Generated first guess in in {:?} ({}B char/s)\n",
                start.elapsed(),
                ((total_chars as f64 / start.elapsed().as_secs_f64()) / 1_000_000_000.0)
                    .smooth_str()
            );
            return guess;
        }
        FirstGuessStrategy::Simple => {
            // count all characters and take the top 5
            let mut char_counts: HashMap<char, usize> = HashMap::new();
            let start = std::time::Instant::now();
            for word in words {
                for c in word.chars() {
                    let count = char_counts.entry(c).or_insert(0);
                    *count += 1;
                }
            }
            println!(
                "Counted {} characters in {:?}",
                total_chars,
                start.elapsed()
            );
            let start = std::time::Instant::now();
            // sort by count
            let mut char_counts: Vec<(char, usize)> = char_counts.into_iter().collect();
            char_counts.sort_by_key(|(_, count)| *count);
            // take the top 5
            let mut guess = String::new();
            for (c, _) in char_counts.iter().rev().take(5) {
                guess.push(*c);
            }
            println!(
                "Generated first guess in in {:?} ({}B char/s)\n",
                start.elapsed(),
                ((total_chars as f64 / start.elapsed().as_secs_f64()) / 1_000_000_000.0)
                    .smooth_str()
            );
            return guess;
        }
    }
}

/// Handles user input for a guess result
fn get_guess_result() -> GuessResult {
    let mut buffer = [Character::Empty; 5];

    for t in ["yellow", "red", "green"] {
        if buffer.iter().all(|c| !matches!(c, Character::Empty)) {
            break;
        }
        println!(
            "Please enter the {t} characters. For non-{t} characters, use '-'",
            t = match t {
                "yellow" => "yellow".yellow(),
                "red" => "red".red(),
                "green" => "green".green(),
                _ => unreachable!(),
            }
        );
        let input = read_line(5);
        for (i, c) in input.chars().enumerate() {
            if c != '-' {
                buffer[i] = match t {
                    "yellow" => Character::Yellow(c),
                    "red" => Character::Red(c),
                    "green" => Character::Green(c),
                    _ => unreachable!(),
                }
            }
        }
    }

    buffer.try_into().unwrap()
}

/// Reads a line from stdin, and returns it as a String. If the line is not the
/// expected length, the user is prompted to try again.
fn read_line(expected_length: usize) -> String {
    let mut buffer = String::new();
    print!(">> ");
    std::io::stdout().flush().unwrap();
    std::io::stdin().read_line(&mut buffer).unwrap();
    buffer = buffer.trim().to_string();
    if buffer.len() != expected_length {
        println!("Please enter exactly {} characters.", expected_length);
        read_line(expected_length)
    } else {
        buffer
    }
}

/// Calculates the result of a guess.
fn calculate_guess_result(word: &String, guess: &String) -> GuessResult {
    if word.len() != guess.len() {
        panic!(
            "Word of length {} and guess of length {}",
            word.len(),
            guess.len()
        );
    }
    let mut result = [Character::Empty; 5];
    for (i, c) in guess.chars().enumerate() {
        if word.contains(c) {
            if word.chars().nth(i).unwrap() == c {
                result[i] = Character::Green(c);
            } else {
                result[i] = Character::Yellow(c);
            }
        } else {
            result[i] = Character::Red(c);
        }
    }

    // ensure no character is empty
    for c in result.iter() {
        if let Character::Empty = c {
            panic!("Empty character in guess result");
        }
    }

    result
}

/// returns the number of words solvable within 5 guesses with the given
/// strategy
fn test_strategy(words: &Vec<String>, strategy: FirstGuessStrategy) -> (i32, String) {
    // a mutex protects the solvable counter, since we use a multithreaded iterator
    let solvable = Mutex::new(0);
    let guess = get_first_guess(&words, strategy);
    words.par_iter().for_each(|word| {
        let mut possible_words = words.clone();
        let mut guess = guess.clone();
        let mut guesses = 5;
        let mut known_info = vec![];
        loop {
            let result = calculate_guess_result(word, &guess);
            known_info.push(result);
            possible_words = filter_using_known_info(&possible_words, &known_info);
            if possible_words.len() == 1 {
                // we lock solvable to the current thread, so we can increment it
                solvable.lock().unwrap().add_assign(1);
                break;
            } // solvable is unlocked here
            guesses -= 1;
            if guesses == 0 {
                break;
            }
            guess = possible_words[0].clone();
        }
    });
    let solvable = *solvable.lock().unwrap();
    (solvable, guess)
}

/// Chooses the optimal strategy for the given word list
fn choose_optimal_strategy(words: &Vec<String>) -> (FirstGuessStrategy, String) {
    println!("Choosing optimal strategy for this word list...");
    let mut results: HashMap<FirstGuessStrategy, (i32, String)> = HashMap::new();

    [
        FirstGuessStrategy::Simple,
        FirstGuessStrategy::PositionAware,
    ]
    .iter()
    .for_each(|strategy| {
        let (solvable, guess) = test_strategy(&words, *strategy);
        results.insert(*strategy, (solvable, guess));
    });

    results.iter().for_each(|(s, w)| {
        println!(
            "{:?} strategy found {} ({}%) words solvable within 5 guesses",
            s,
            w.0,
            (w.0 as f64 / words.len() as f64 * 100.0).smooth_str()
        )
    });
    let winner = results.iter().max_by_key(|(_, (count, _))| *count).unwrap();

    (winner.0.clone(), winner.1 .1.clone())
}
