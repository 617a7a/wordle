use blake3::hash;
use bytecheck::CheckBytes;
use colored::Colorize;
use directories::ProjectDirs;
use rand::Rng;
use rayon::prelude::*;
use rkyv::{Archive, Deserialize, Serialize};
use smooth::Smooth;
use spinoff::{spinners, Spinner};
use std::{
    collections::HashMap,
    io::{Read, Write},
};

const WORDS: &str = include_str!("../../wordle/src/words.txt");

struct GuessResult([Character; 5]);

impl std::fmt::Debug for GuessResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            self.0
                .iter()
                .map(|c| format!("{:?}", c))
                .collect::<String>()
        )
    }
}

#[derive(Debug, Clone)]
struct ScoredWord {
    word: String,
    score: usize,
}

#[derive(Archive, Deserialize, Serialize)]
#[archive_attr(derive(CheckBytes))]
struct WordListCache {
    strats: HashMap<Vec<u8>, (Strategy, String)>,
}

fn main() {
    let cache_dir = ProjectDirs::from("com", "617a7a", "wordle")
        .expect("Could not find config directory")
        .cache_dir()
        .to_str()
        .expect("Could not convert config directory to string")
        .to_string();

    // create the cache directory if it doesn't exist
    std::fs::create_dir_all(&cache_dir).expect("Could not create cache directory");

    // open read-write, create if it doesn't exist at cache_dir/strategies
    let mut cache_file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(format!("{}/strategies", cache_dir))
        .expect("Could not open cache file");

    let mut bytes = vec![];

    cache_file
        .read_to_end(&mut bytes)
        .expect("Could not read cache file");

    let cache: WordListCache = rkyv::from_bytes(&bytes).unwrap_or(WordListCache {
        strats: HashMap::new(),
    });

    let mut known_info: Vec<GuessResult> = vec![];
    let mut words: Vec<ScoredWord> = WORDS
        .split_whitespace()
        .map(|s| ScoredWord {
            word: s.to_string(),
            score: 1,
        })
        .collect();

    let words_digest = hash(WORDS.as_bytes());

    let first_guess: String;

    if let Some(strat) = cache.strats.get(&words_digest.as_bytes().to_vec()) {
        println!(
            "Using {} strategy from cache at {}/strategies for wordset {}",
            format!("{:?}", strat.0).magenta(),
            cache_dir,
            words_digest.to_hex().cyan()
        );
        first_guess = strat.1.clone();
    } else {
        println!(
            "{}",
            format!(
                "No cached strategy found, generating one for wordset {}",
                words_digest.to_hex()
            )
            .black()
        );
        let (strat, fw) = choose_optimal_strategy(&words);

        let mut cache = cache;
        cache
            .strats
            .insert(words_digest.as_bytes().to_vec(), (strat, fw.clone()));

        first_guess = fw;

        cache_file
            .write_all(
                &rkyv::to_bytes::<WordListCache, 4096>(&cache).expect("Could not serialise cache"),
            )
            .expect("Could not write to cache file");
        println!(
            "{}",
            format!("Cached strategy in {}/strategies", cache_dir).black()
        );
    }

    let mut last_guess = first_guess.clone();

    for i in 0..5 {
        if i == 0 {
            println!("\n{}", "TIPS:".bold());
            println!(" - Leave a field empty to autofill all empty letters with that colour");
            println!(" - If you type less than 5 letters, we'll replace the rest with dashes");

            // for our first guess, we have no information, so we just guess the word
            // not as an actual word, but as the top 5 letters in the word list by frequency
            println!("\nFirst guess is {}!", first_guess.blue());
        } else {
            // after the first guess, we get input from the user which we can use to refine
            // our guess
            let guess_result = get_guess_result(&last_guess);
            known_info.push(guess_result);
            let start = std::time::Instant::now();
            let filtered_results = filter_using_known_info(&words, &known_info);
            let elapsed = start.elapsed();
            let total_chars = filtered_results.iter().map(|s| s.word.len()).sum::<usize>();

            println!(
                "\n{} Found {} possible {}",
                format!(
                    "[{:?}, {} char/s]",
                    elapsed,
                    (total_chars as f64 / start.elapsed().as_secs_f64()).smooth_str()
                )
                .black(),
                filtered_results.len(),
                if filtered_results.len() == 1 {
                    "word"
                } else {
                    "words"
                },
            );
            let start = std::time::Instant::now();
            words = optimise_results(filtered_results, &known_info);
            let elapsed = start.elapsed();
            println!(
                "{} Scored & reordered results",
                format!(
                    "[{:?}, {} char/s]",
                    elapsed,
                    (total_chars as f64 / start.elapsed().as_secs_f64()).smooth_str()
                )
                .black(),
            );

            last_guess = words[0].word.clone();

            let total_score = words.par_iter().map(|sw| sw.score).sum::<usize>();

            if words.len() < 5 || i == 4 {
                let fmttd_list = words
                    .iter()
                    .map(|sw| {
                        format!(
                            "  - {} ({}%)",
                            sw.word.blue(),
                            (100.0 * (sw.score as f64) / (total_score as f64)).smooth_str()
                        )
                    })
                    .collect::<Vec<String>>()
                    .join("\n");
                println!("Try one of these: \n{}", fmttd_list);
            } else {
                let sw = &words[0];
                println!(
                    "Try {} ({}%)",
                    sw.word.blue(),
                    (100.0 * (sw.score as f64) / (total_score as f64)).smooth_str()
                );
            }
        }
        if i != 4 {
            println!("\n{}", format!("Guess {} of 5", i + 1).black());
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
fn filter_using_known_info(
    words: &Vec<ScoredWord>,
    known_info: &Vec<GuessResult>,
) -> Vec<ScoredWord> {
    // we have a list of words, and we know some information about the word we're
    // looking for we process the words finding possible words that match
    // **all** the known information
    words
        .iter()
        .filter(|sw| {
            known_info.iter().all(|guess| {
                guess.0.iter().enumerate().all(|(i, c)| match c {
                    // word contains all yellow characters
                    Character::Yellow(t) => {
                        sw.word.contains(*t) && sw.word.chars().nth(i).unwrap() != *t
                    }
                    // word contains all green characters in the correct position
                    Character::Green(t) => sw.word.chars().nth(i).unwrap() == *t,
                    // word doesn't contain any red characters
                    Character::Red(t) => !sw.word.contains(*t),
                    Character::Empty => unreachable!("Empty character in known_info"),
                })
            })
        })
        .map(|sw| sw.clone())
        .collect()
}

/// reorders a wordlist to optimise the next guess using the strategy
fn optimise_results(results: Vec<ScoredWord>, known_info: &Vec<GuessResult>) -> Vec<ScoredWord> {
    // if the length is 0, no optimisation is required
    if results.len() == 0 {
        return results;
    }

    // at this stage, the filter has ensured that any red characters are not in the
    // word, and all green characters are already in their correct positions.
    // we therefore score based upon the yellow characters exclusively,
    // so the list of results is sorted to lower the maximum guesses to find the
    // word

    // this is done by scoring each word based on the frequency of the yellow
    // characters

    // example: we make these two guesses:
    // [Red(D), Green(R), Red(U), Yellow(N), Red(K)]
    // [Red(F), Red(I), Yellow(G), Red(H), Red(T)]
    // which narrows the wordlist down to:
    // groan, green, grown

    // we can identify that the first character has to be 'g', the second is 'r' and
    // the last is 'n' the differences between the words are therefore the third
    // and fourth characters:   'o' and 'e'
    // for position 3, 'o' is the most common character, so words with 'o' in
    // position 3 are more likely to be the word than words with 'e' in position
    // 3 "grown" and "groan" are equally likely to be the word, as their uniqueness
    // is the same, but "green" is less likely

    // ALGORITHM:
    // 1. count the frequency of each character in each position using a
    //    [[usize; 26]; 5], ensuring to ignore any green or red characters
    // 2. score each word based on the frequency of the yellow characters
    // 3. sort the words by their score

    let frequencies: [[usize; 26]; 5] = results.iter().fold(
        [[0; 26], [0; 26], [0; 26], [0; 26], [0; 26]],
        |mut acc, sw| {
            for (i, c) in sw.word.chars().enumerate() {
                acc[i][c as usize - 97] += 1;
            }
            acc
        },
    );

    let mut scored_words = results
        .par_iter()
        .map(|sw| {
            let mut score = 1;
            for (i, c) in sw.word.chars().enumerate() {
                // if all the known info for this position is yellow, we can score
                if known_info
                    .iter()
                    .all(|guess| matches!(guess.0[i], Character::Yellow(_)))
                {
                    score += frequencies[i][c as usize - 97];
                }
            }
            ScoredWord {
                word: sw.word.clone(),
                score,
            }
        })
        .collect::<Vec<ScoredWord>>();
    scored_words.sort_by(|a, b| b.score.cmp(&a.score));
    scored_words
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Archive, Deserialize, Serialize)]
#[archive_attr(derive(CheckBytes))]
enum Strategy {
    FrequencySimple,
    FrequencyPositionAware,
    Random,
}

/// Returns the optimal starting guess for the wordset
fn get_first_guess(words: &Vec<ScoredWord>, strategy: Strategy) -> String {
    match strategy {
        Strategy::FrequencyPositionAware => {
            // our first guess is constructed off the most common character in each position
            let frequencies: [[usize; 26]; 5] = words.iter().fold(
                [[0; 26], [0; 26], [0; 26], [0; 26], [0; 26]],
                |mut acc, sw| {
                    for (i, c) in sw.word.chars().enumerate() {
                        acc[i][c as usize - 97] += 1;
                    }
                    acc
                },
            );

            // find the most likely character in each position
            let mut guess = String::new();
            for freq in frequencies.iter() {
                let mut max = 0;
                let mut max_index = 0;
                for (i, count) in freq.iter().enumerate() {
                    if *count > max {
                        max = *count;
                        max_index = i;
                    }
                }
                guess.push((max_index + 97) as u8 as char);
            }

            return guess;
        }
        Strategy::FrequencySimple => {
            // count all characters and take the top 5
            let mut char_counts: [usize; 26] = [0; 26];
            for sw in words {
                for c in sw.word.chars() {
                    char_counts[c as usize - 97] += 1;
                }
            }

            // sort by count
            let mut char_counts: Vec<(usize, char)> = char_counts
                .iter()
                .enumerate()
                .map(|(i, count)| (*count, (i + 97) as u8 as char))
                .collect();
            char_counts.sort_by(|a, b| b.0.cmp(&a.0));

            // take the top 5
            let mut guess = String::new();
            for (_, c) in char_counts.iter().take(5) {
                guess.push(*c);
            }

            return guess;
        }
        Strategy::Random => {
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
    let mut buffer = GuessResult([Character::Empty; 5]);

    for t in ["yellow", "red", "green"] {
        if buffer.0.iter().all(|c| !matches!(c, Character::Empty)) {
            break;
        } else if t == "green" {
            let last_guess: Vec<char> = last_guess.chars().collect();
            // replace all empty characters with green characters from the previous guess
            for (i, c) in buffer.0.iter_mut().enumerate() {
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
            for (i, c) in buffer.0.iter_mut().enumerate() {
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
                    buffer.0[i] = match t {
                        "yellow" => Character::Yellow(c),
                        "red" => Character::Red(c),
                        "green" => Character::Green(c),
                        _ => unreachable!(),
                    }
                }
            }
        }
    }

    print!("You have entered {:?}. Correct? (y): ", buffer);
    let mut key = String::new();
    std::io::stdout().flush().unwrap();
    std::io::stdin().read_line(&mut key).unwrap();
    key = key.trim().to_string();

    if key == "y" || key == "" {
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

    // ensure string is lowercase a-z or -
    if !buffer.chars().all(|c| matches!(c, 'a'..='z' | '-')) {
        println!("Please enter only lowercase letters or '-'.");
        read_line(expected_length, guess)
    } else if buffer.len() != expected_length {
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

    GuessResult(result)
}

/// returns the number of words solvable within 5 guesses with the given
/// strategy
fn test_strategy(words: &Vec<ScoredWord>, strategy: Strategy) -> (i32, String) {
    let guess = get_first_guess(words, strategy);
    let solvable = words
        .par_iter()
        .map(|sw| {
            let mut possible_words = words.clone();
            let mut guess = guess.clone();
            let mut guesses = 5;
            let mut known_info = vec![];
            loop {
                let result = calculate_guess_result(&sw.word, &guess);
                known_info.push(result);
                possible_words = filter_using_known_info(&possible_words, &known_info);
                possible_words = optimise_results(possible_words, &known_info);
                if possible_words[0].word == *sw.word {
                    return 1;
                }
                guesses -= 1;
                if guesses == 0 {
                    return 0;
                }
                guess = possible_words[0].word.clone();
            }
        })
        .sum();
    (solvable, guess)
}

/// Chooses the optimal strategy for the given word list
fn choose_optimal_strategy(words: &Vec<ScoredWord>) -> (Strategy, String) {
    let mut sp = Spinner::new(
        spinners::Aesthetic,
        "Choosing optimal strategy for this word list",
        None,
    );
    let mut results: HashMap<Strategy, (i32, String)> = HashMap::new();

    let start = std::time::Instant::now();

    let options = [
        Strategy::FrequencySimple,
        Strategy::FrequencyPositionAware,
        Strategy::Random,
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
        .collect::<Vec<(Strategy, (i32, String))>>()
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
        (100.0 * (winner.1 .0 as f64) / (words.len() as f64)).smooth_str(),
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
