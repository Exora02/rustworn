// Thin binary wrapper. All real logic lives in the library crate so
// integration tests and (eventually) a server headless mode can both
// boot the same Bevy app.

fn main() {
    rustworn::run();
}
