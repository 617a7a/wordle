# Wordle CLI

CLI Wordle, in Rust. Yes, there are tonnes of other implementations already. Yes, I could've used other crates to do this. But that's no fun 🎢 

This is a monorepo with a Wordle game you can play in the terminal, as well as a solver alongside it. The solver was overengineered, but it works about 80% of the time. The solver will choose a strategy by solving every single word on the list on startup. Does a Wordle solver need to be multithreaded? Probably not. Oh well. 

## Running it

Make sure you have Rust installed. You can use [Rustup](https://rustup.rs) for that. 

```
cargo run -r -p {wordle/solver}
```

or you can install each one to your path

```
cargo install --path {wordle/solver}
```
