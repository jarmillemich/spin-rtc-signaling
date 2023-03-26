use rand::seq::IteratorRandom;

static NAME_PIECES: [&str; 3] = [
    include_str!("names/names_1.txt"),
    include_str!("names/names_2.txt"),
    include_str!("names/names_3.txt"),
];

/// Generates a random name consisting of three words from our wordlists
pub fn generate_name() -> String {
    let mut rng = rand::thread_rng();
    NAME_PIECES.map(|list| list.lines().choose(&mut rng).unwrap()).join(" ")
}