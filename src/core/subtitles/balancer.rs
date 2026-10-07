// -*- coding: utf-8 -*-
//! Dedicated native Rust subtitle balancing and linear distribution module.
//!
//! Single responsibility:
//! - Identify syntactic boundaries by punctuation (commas, periods, pauses).
//! - Split long sentences exactly in half (minimizing character count difference between parts).
//! - Distribute timestamps proportionally and strictly linearly, avoiding overlap.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TimedWord {
    pub text: String,
    pub start: f64,
    pub end: f64,
}

impl TimedWord {
    pub fn new(text: impl Into<String>, start: f64, end: f64) -> Self {
        Self {
            text: text.into(),
            start,
            end,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SubtitleChunk {
    pub start: f64,
    pub end: f64,
    pub text: String,
    pub confidence: f32,
    #[serde(default)]
    pub words: Vec<TimedWord>,
}

/// Identifies punctuation marking the end of a sentence or clause (syntactic pause).
/// Any comma, period, ellipsis, semicolon, or colon serves as a cut boundary.
pub fn is_clause_terminal(word: &str) -> bool {
    let cleaned = word.trim().trim_end_matches(['"', '\'', '»', '”', '’']);
    for p in &[".", "!", "?", "...", ",", ";", ":"] {
        if cleaned.ends_with(p) {
            return true;
        }
    }
    false
}

/// Commas and pauses that separate intermediate clauses.
pub fn is_soft_pause(word: &str) -> bool {
    let cleaned = word.trim().trim_end_matches(['"', '\'', '»', '”', '’']);
    for p in &[",", ";", ":"] {
        if cleaned.ends_with(p) {
            return true;
        }
    }
    false
}

fn words_to_string(words: &[TimedWord]) -> String {
    words
        .iter()
        .map(|w| w.text.as_str())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Recursively splits a list of words into k balanced parts.
fn split_into_k(words: &[TimedWord], k: usize, max_chars: usize) -> Vec<Vec<TimedWord>> {
    if k <= 1 || words.len() <= 1 {
        return vec![words.to_vec()];
    }

    let full_text = words_to_string(words);
    let total_len = full_text.chars().count();
    let k_target = total_len as f64 / k as f64;

    let mut best_split = 1usize;
    let mut best_score = f64::INFINITY;

    for i in 0..(words.len() - 1) {
        let part1_words = &words[..=i];
        let part2_words = &words[(i + 1)..];

        let part1_text = words_to_string(part1_words);
        let part2_text = words_to_string(part2_words);

        let part1_len = part1_text.chars().count();
        let part2_len = part2_text.chars().count();

        // 1. Absolute distance relative to ideal linear average
        let dist_to_target = (part1_len as f64 - k_target).abs();

        // 2. If splitting into 2 parts, penalize direct asymmetry between them
        let symmetry_penalty = if k == 2 {
            (part1_len as f64 - part2_len as f64).abs() * 1.5
        } else {
            0.0
        };

        // 3. Severe penalty if exceeding max_chars limit
        let mut overflow_penalty = 0.0;
        if part1_len > max_chars {
            overflow_penalty += (part1_len - max_chars) as f64 * 150.0;
        }
        if k == 2 && part2_len > max_chars {
            overflow_penalty += (part2_len - max_chars) as f64 * 150.0;
        }

        // 4. Avoid leaving a single short orphan word at edges if sentence is long
        let mut orphan_penalty = 0.0;
        if words.len() >= 4 {
            if (i + 1) == 1 && words[0].text.chars().count() <= 3 {
                orphan_penalty += 35.0;
            }
            if (words.len() - (i + 1)) == 1 && words.last().unwrap().text.chars().count() <= 3 {
                orphan_penalty += 35.0;
            }
        }

        let score = dist_to_target + symmetry_penalty + overflow_penalty + orphan_penalty;
        if score < best_score {
            best_score = score;
            best_split = i + 1;
        }
    }

    let first = words[..best_split].to_vec();
    let mut rest = split_into_k(&words[best_split..], k - 1, max_chars);
    let mut result = vec![first];
    result.append(&mut rest);
    result
}

/// Splits a clause in a mathematically balanced and linear manner.
/// Ensures that:
/// 1. The sentence is split exactly in half (smallest character difference between parts).
/// 2. No part exceeds max_chars (if possible).
/// 3. Symmetrical and aesthetically pleasing parts for comfortable reading.
pub fn calculate_optimal_split(words_data: &[TimedWord], max_chars: usize) -> Vec<Vec<TimedWord>> {
    if words_data.is_empty() {
        return Vec::new();
    }

    let full_text = words_to_string(words_data);
    let total_len = full_text.chars().count();
    if total_len <= max_chars {
        return vec![words_data.to_vec()];
    }

    let num_parts = (total_len as f64 / max_chars as f64).ceil() as usize;
    let num_parts = num_parts.max(2);
    let total_words = words_data.len();

    if total_words <= 1 {
        return vec![words_data.to_vec()];
    }

    let mut k = num_parts;
    let mut best_result = None;

    while k <= total_words {
        let parts = split_into_k(words_data, k, max_chars);
        let max_part_len = parts
            .iter()
            .map(|p| words_to_string(p).chars().count())
            .max()
            .unwrap_or(0);

        if max_part_len <= max_chars || k == total_words {
            best_result = Some(parts);
            break;
        }
        k += 1;
    }

    best_result.unwrap_or_else(|| vec![words_data.to_vec()])
}

/// Generates subtitle chunks with consistent timestamps.
/// If the model provided identical or collapsed timestamps between words,
/// interpolates duration proportionally by character count of each part.
pub fn build_interpolated_chunks(parts: &[Vec<TimedWord>]) -> Vec<SubtitleChunk> {
    let mut chunks: Vec<SubtitleChunk> = Vec::new();
    if parts.is_empty() {
        return chunks;
    }

    for p in parts {
        if p.is_empty() {
            continue;
        }

        let raw_start = p.first().unwrap().start;
        let raw_end = p.last().unwrap().end;
        let text = words_to_string(p);

        let start = (raw_start * 1000.0).round() / 1000.0;
        let end = ((raw_start + 0.2).max(raw_end) * 1000.0).round() / 1000.0;

        chunks.push(SubtitleChunk {
            start,
            end,
            text,
            confidence: 0.99,
            words: p.clone(),
        });
    }

    // Check if there are chunks with same start timestamp or temporal collision
    for idx in 0..chunks.len().saturating_sub(1) {
        if chunks[idx + 1].start <= chunks[idx].start || chunks[idx].end >= chunks[idx + 1].end {
            let total_window_start = chunks[idx].start;
            let total_window_end = chunks[idx].end.max(chunks[idx + 1].end);
            let c1_len = chunks[idx].text.chars().count();
            let c2_len = chunks[idx + 1].text.chars().count();
            let total_chars = (c1_len + c2_len).max(1);
            let ratio1 = c1_len as f64 / total_chars as f64;
            let window_dur = (total_window_end - total_window_start).max(0.4);

            let split_time = ((total_window_start + (window_dur * ratio1)) * 1000.0).round() / 1000.0;
            chunks[idx].end = split_time;
            chunks[idx + 1].start = split_time;
        }
    }

    chunks
}

/// Groups words into clauses delimited by punctuation (commas, periods) or silence,
/// and then splits each long clause exactly in half.
pub fn group_words_into_balanced_clauses(
    words_data: &[TimedWord],
    max_chars: usize,
    max_silence: f64,
) -> Vec<SubtitleChunk> {
    if words_data.is_empty() {
        return Vec::new();
    }

    // Step 1: Delimit clauses by terminal and syntactic pause punctuation
    let mut clauses: Vec<Vec<TimedWord>> = Vec::new();
    let mut current_clause: Vec<TimedWord> = Vec::new();

    for (i, word) in words_data.iter().enumerate() {
        let trimmed_text = word.text.trim();
        if trimmed_text.is_empty() {
            continue;
        }

        current_clause.push(TimedWord {
            text: trimmed_text.to_string(),
            start: word.start,
            end: word.end,
        });

        let is_term = is_clause_terminal(trimmed_text);
        let mut is_pause = false;
        if i < words_data.len() - 1 {
            let next_start = words_data[i + 1].start;
            // Significant silence between utterances
            if next_start - word.end >= max_silence {
                is_pause = true;
            }
        }

        // If terminal punctuation/comma or expressive pause reached
        if is_term || is_pause {
            clauses.push(std::mem::take(&mut current_clause));
        }
    }

    if !current_clause.is_empty() {
        clauses.push(current_clause);
    }

    // Step 2: Merge ultra-short orphan fragments
    // If a clause has 1 or 2 very short words without punctuation, merge with adjacent clause
    let mut merged_clauses: Vec<Vec<TimedWord>> = Vec::new();
    let mut idx = 0;
    while idx < clauses.len() {
        let c = &clauses[idx];
        let c_text = words_to_string(c);
        let c_len = c_text.chars().count();

        // Short clause that fits with the next one
        let threshold = (8.0f64).max(max_chars as f64 * 0.28) as usize;
        if c_len < threshold && idx < clauses.len() - 1 {
            let next_c = &clauses[idx + 1];
            let next_text = words_to_string(next_c);
            let next_len = next_text.chars().count();

            if c_len + 1 + next_len <= max_chars {
                let mut combined = c.clone();
                combined.extend(next_c.clone());
                merged_clauses.push(combined);
                idx += 2;
                continue;
            }
        }

        merged_clauses.push(c.clone());
        idx += 1;
    }

    // Step 3: Each clause is split linearly at the most balanced midpoint
    let mut results: Vec<SubtitleChunk> = Vec::new();
    for c in merged_clauses {
        let parts = calculate_optimal_split(&c, max_chars);
        let chunks = build_interpolated_chunks(&parts);
        results.extend(chunks);
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clause_terminal_detection() {
        assert!(is_clause_terminal("olá,"));
        assert!(is_clause_terminal("Sniplic."));
        assert!(is_clause_terminal("fim!"));
        assert!(is_clause_terminal("dúvida?"));
        assert!(is_clause_terminal("continua..."));
        assert!(is_clause_terminal("item:"));
        assert!(is_clause_terminal("texto;"));
        assert!(is_clause_terminal("\"aspas.\""));
        assert!(!is_clause_terminal("palavra"));
        assert!(!is_clause_terminal("video editor"));
    }

    #[test]
    fn test_short_clause_no_split() {
        let words = vec![
            TimedWord::new("Olá", 0.0, 0.4),
            TimedWord::new("mundo", 0.4, 0.9),
            TimedWord::new("Sniplic!", 0.9, 1.5),
        ];
        let split = calculate_optimal_split(&words, 36);
        assert_eq!(split.len(), 1);
        assert_eq!(split[0].len(), 3);
    }

    #[test]
    fn test_long_sentence_balanced_split() {
        // ~70-character sentence: split perfectly into 3 parts where each has <= 36 characters
        let words = vec![
            TimedWord::new("Este", 0.0, 0.3),
            TimedWord::new("é", 0.3, 0.5),
            TimedWord::new("um", 0.5, 0.7),
            TimedWord::new("vídeo", 0.7, 1.1),
            TimedWord::new("completamente", 1.1, 1.8),
            TimedWord::new("profissional", 1.8, 2.5),
            TimedWord::new("editado", 2.5, 3.0),
            TimedWord::new("no", 3.0, 3.2),
            TimedWord::new("Sniplic", 3.2, 3.8),
            TimedWord::new("Desktop.", 3.8, 4.5),
        ];
        let split = calculate_optimal_split(&words, 36);
        assert_eq!(split.len(), 3);
        for (idx, part) in split.iter().enumerate() {
            let part_len = words_to_string(part).chars().count();
            assert!(part_len <= 36, "Part {} exceeded limit: {}", idx, part_len);
        }

        // ~48-character sentence: split perfectly into 2 parts
        let words2 = vec![
            TimedWord::new("Este", 0.0, 0.3),
            TimedWord::new("é", 0.3, 0.5),
            TimedWord::new("um", 0.5, 0.7),
            TimedWord::new("vídeo", 0.7, 1.1),
            TimedWord::new("editado", 1.1, 1.6),
            TimedWord::new("no", 1.6, 1.8),
            TimedWord::new("Sniplic", 1.8, 2.3),
            TimedWord::new("Desktop.", 2.3, 3.0),
        ];
        let split2 = calculate_optimal_split(&words2, 36);
        assert_eq!(split2.len(), 2);
        let p1_len = words_to_string(&split2[0]).chars().count();
        let p2_len = words_to_string(&split2[1]).chars().count();
        assert!(p1_len <= 36);
        assert!(p2_len <= 36);
        let diff = (p1_len as isize - p2_len as isize).abs();
        assert!(diff <= 15, "Difference too high: {}", diff);
    }

    #[test]
    fn test_group_words_respects_punctuation_and_silence() {
        let words = vec![
            TimedWord::new("Primeira", 0.0, 0.4),
            TimedWord::new("frase,", 0.4, 0.8),
            TimedWord::new("segunda", 0.85, 1.2),
            TimedWord::new("frase.", 1.2, 1.6),
            // Silence pause of 1.5s (> 0.9s)
            TimedWord::new("Terceira", 3.1, 3.5),
            TimedWord::new("frase.", 3.5, 4.0),
        ];
        let chunks = group_words_into_balanced_clauses(&words, 36, 0.9);
        assert!(chunks.len() >= 2);
        // First block must contain the first clause
        assert!(chunks[0].text.contains("Primeira"));
    }
}
