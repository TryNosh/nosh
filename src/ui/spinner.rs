//! Fancy AI spinner with random fun terms.

use indicatif::{ProgressBar, ProgressStyle};
use std::time::Duration;

/// Fun terms to show while AI is thinking
const THINKING_TERMS: &[&str] = &[
    // Classic
    "Thinking",
    "Pondering",
    "Contemplating",
    "Reasoning",
    "Calculating",
    // Culinary
    "Brewing",
    "Cooking",
    "Baking",
    "Simmering",
    "Marinating",
    "Fermenting",
    // Crafty
    "Crafting",
    "Weaving",
    "Knitting",
    "Forging",
    "Sculpting",
    // Magical
    "Conjuring",
    "Summoning",
    "Manifesting",
    "Enchanting",
    "Divining",
    "Channeling",
    // Dreamy
    "Dreaming",
    "Imagining",
    "Envisioning",
    "Fantasizing",
    // Scientific
    "Synthesizing",
    "Analyzing",
    "Computing",
    "Extrapolating",
    "Hypothesizing",
    // Silly made-up
    "Cogitating",
    "Brain-wrangling",
    "Thought-smithing",
    "Idea-farming",
    "Neuron-tickling",
    "Mind-gardening",
    "Synapse-juggling",
    "Logic-knitting",
    "Wisdom-distilling",
    "Insight-mining",
    "Notion-herding",
    "Concept-wrangling",
    "Brainstorming",
    "Noodling",
    "Percolating",
    "Ruminating",
    "Musing",
    "Mulling",
];

/// Spinner frames - a sparkling star effect
const SPINNER_FRAMES: &[&str] = &["✶", "✷", "✸", "✹", "✺", "✹", "✸", "✷"];

/// Create a fancy AI spinner with a random thinking term
pub fn create() -> ProgressBar {
    let term = random_term();
    let spinner = ProgressBar::new_spinner();

    spinner.set_style(
        ProgressStyle::default_spinner()
            .tick_strings(SPINNER_FRAMES)
            .template("{spinner:.cyan} {msg}")
            .unwrap(),
    );
    spinner.set_message(format!("{}...", term));
    spinner.enable_steady_tick(Duration::from_millis(80));

    spinner
}

/// Get a random thinking term
fn random_term() -> &'static str {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};

    // Use RandomState to get a random index
    let random = RandomState::new().build_hasher().finish() as usize;
    THINKING_TERMS[random % THINKING_TERMS.len()]
}
