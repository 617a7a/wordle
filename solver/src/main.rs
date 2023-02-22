use colored::Colorize;
use rand::Rng;
use rayon::prelude::*;
use smooth::Smooth;
use spinoff::{spinners, Spinner};
use std::{collections::HashMap, io::Write, ops::AddAssign, sync::Mutex};

const WORDS: &str = include_str!("../../wordle/src/words.txt");

type GuessResult = [Character; 5];

fn main() {
    let mut words: Vec<String> = WORDS.split_whitespace().map(|s| s.to_string()).collect();
    let (_, first_guess) = choose_optimal_strategy(&words);

    let mut known_info: Vec<GuessResult> = vec![];

    let mut last_guess = first_guess.clone();

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
            let guess_result = get_guess_result(&last_guess);
            known_info.push(guess_result);
            let start = std::time::Instant::now();
            let filtered_results = filter_using_known_info(&words, &known_info);
            last_guess = filtered_results[0].clone();
            println!(
                "{} Found {} possible {}",
                format!(
                    "[{:?}, {} char/s]",
                    start.elapsed(),
                    ((filtered_results.len() as f64) / (start.elapsed().as_secs_f64()))
                        .smooth_str()
                )
                .black(),
                filtered_results.len(),
                if filtered_results.len() == 1 {
                    "word"
                } else {
                    "words"
                },
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
                    Character::Yellow(t) => word.contains(*t) && word.chars().nth(i).unwrap() != *t,
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
    FrequencySimple,
    FrequencyPositionAware,
    Random,
}

/// Returns the optimal starting guess for the wordset
fn get_first_guess(words: &Vec<String>, strategy: FirstGuessStrategy) -> String {
    match strategy {
        FirstGuessStrategy::FrequencyPositionAware => {
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

            // find the most likely character in each position
            let guess = frequencies.iter().fold(String::new(), |mut acc, freq| {
                let (c, _) = freq.iter().max_by_key(|(_, count)| *count).unwrap();
                acc.push(*c);
                acc
            });

            return guess;
        }
        FirstGuessStrategy::FrequencySimple => {
            // count all characters and take the top 5
            let mut char_counts: HashMap<char, usize> = HashMap::new();
            for word in words {
                for c in word.chars() {
                    let count = char_counts.entry(c).or_insert(0);
                    *count += 1;
                }
            }

            // sort by count
            let mut char_counts: Vec<(char, usize)> = char_counts.into_iter().collect();
            char_counts.sort_by_key(|(_, count)| *count);
            // take the top 5
            let mut guess = String::new();
            for (c, _) in char_counts.iter().rev().take(5) {
                guess.push(*c);
            }

            return guess;
        }
        FirstGuessStrategy::Random => {
            // create 5 random characters
            let mut rng = rand::thread_rng();
            let mut guess = String::new();
            for _ in 0..5 {
                guess.push(rng.gen_range('a'..='z'));
            }
            return guess;
        }
    }
}

/// Handles user input for a guess result
fn get_guess_result(last_guess: &String) -> GuessResult {
    let mut buffer = [Character::Empty; 5];

    for t in ["yellow", "red", "green"] {
        if buffer.iter().all(|c| !matches!(c, Character::Empty)) {
            break;
        } else if t == "green" {
            let last_guess: Vec<char> = last_guess.chars().collect();
            // replace all empty characters with green characters from the previous guess
            for (i, c) in buffer.iter_mut().enumerate() {
                if matches!(c, Character::Empty) {
                    *c = Character::Green(last_guess[i]);
                }
            }
            break;
        }
        println!(
            "Enter the {t} characters. For non-{t} characters, use '-':",
            t = match t {
                "yellow" => "yellow".yellow(),
                "red" => "red".red(),
                "green" => "green".green(),
                _ => unreachable!(),
            }
        );
        let input = read_line(5, last_guess);

        if input.len() == 0 {
            // special case for empty input, we assume all empty characters are of the given
            // type
            for (i, c) in buffer.iter_mut().enumerate() {
                if matches!(c, Character::Empty) {
                    let c2 = last_guess.chars().nth(i).unwrap();
                    *c = match t {
                        "yellow" => Character::Yellow(c2),
                        "red" => Character::Red(c2),
                        "green" => Character::Green(c2),
                        _ => unreachable!(),
                    }
                }
            }
        } else {
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
    }

    let fmttd = buffer
        .iter()
        .map(|c| match c {
            Character::Yellow(c) => c.to_string().yellow(),
            Character::Green(c) => c.to_string().green(),
            Character::Red(c) => c.to_string().red(),
            Character::Empty => "-".into(),
        })
        .map(|s| s.to_string())
        .collect::<String>();

    print!("You have entered {}. Correct? (y/n): ", fmttd);
    let mut key = String::new();
    std::io::stdout().flush().unwrap();
    std::io::stdin().read_line(&mut key).unwrap();
    key = key.trim().to_string();

    if key == "y" {
        buffer
    } else {
        get_guess_result(last_guess)
    }
}

/// Reads a line from stdin, and returns it as a String. If the line is not the
/// expected length, the user is prompted to try again.
fn read_line(expected_length: usize, guess: &String) -> String {
    let mut buffer = String::new();
    print!(">> ");
    std::io::stdout().flush().unwrap();
    std::io::stdin().read_line(&mut buffer).unwrap();
    buffer = buffer.trim().to_string();

    if buffer == "exit" {
        println!("Exiting...");
        std::process::exit(0);
    }

    if buffer.len() != expected_length {
        if buffer.len() == 0 {
            buffer
        } else if buffer.len() < expected_length {
            buffer.push_str(&"-".repeat(expected_length - buffer.len()));
            buffer
        } else {
            println!("Please enter exactly {} characters.", expected_length);
            read_line(expected_length, guess)
        }
    } else {
        buffer
    }
}

/// Calculates the result of a guess.
fn calculate_guess_result(word: &String, guess: &String) -> GuessResult {
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
            if possible_words[0] == *word {
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
    let mut sp = Spinner::new(
        spinners::Aesthetic,
        "Choosing optimal strategy for this word list",
        None,
    );
    let mut results: HashMap<FirstGuessStrategy, (i32, String)> = HashMap::new();

    let start = std::time::Instant::now();

    let options = [
        FirstGuessStrategy::FrequencySimple,
        FirstGuessStrategy::FrequencyPositionAware,
        FirstGuessStrategy::Random,
    ];

    options
        .iter()
        .enumerate()
        .map(|(i, s)| {
            sp.update_text(format!(
                "{} Testing {} strategy",
                format!("[{}/{}]", i + 1, options.len()).black(),
                format!("{:?}", s).magenta()
            ));
            (s.clone(), test_strategy(words, s.clone()))
        })
        .collect::<Vec<(FirstGuessStrategy, (i32, String))>>()
        .iter()
        .for_each(|(s, (count, guess))| {
            results.insert(s.clone(), (*count, guess.clone()));
        });

    let winner = results.iter().max_by_key(|(_, (count, _))| *count).unwrap();

    let total_words = words.len() * options.len();

    sp.info(&format!(
        "{} Optimal strategy is {} with {}/{} solvable words ({}%)\n  {}",
        format!("[{:?}]", start.elapsed()).black(),
        format!("{:?}", winner.0).magenta(),
        winner.1 .0,
        words.len(),
        (winner.1 .0 as f64 * 100.0 / words.len() as f64).smooth_str(),
        format!(
            "Solved {} words using {} different strategies ({} wps)",
            total_words,
            options.len(),
            (total_words as f64 / start.elapsed().as_secs_f64()).smooth_str()
        )
        .black()
    ));

    (winner.0.clone(), winner.1 .1.clone())
}
