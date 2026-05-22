#[allow(dead_code)]
use std::collections::BinaryHeap;
use crate::config::cache_dir;

#[allow(dead_code)]
struct HuffmanNode {
    weight: i32,
    children: Vec<usize>, // indices into the arena
}

/// Generate `n` unique hints from `alphabet` using a Huffman coding scheme.
/// Returns hints sorted by length (shortest first), all unique.
pub fn generate_hints(alphabet: &[String], n: usize) -> Vec<String> {
    if let Some(cached) = read_cache(alphabet, n) {
        return cached;
    }
    if n == 0 || alphabet.is_empty() {
        return vec![];
    }
    if n <= alphabet.len() {
        return alphabet[..n].to_vec();
    }

    let arity = alphabet.len();
    let mut arena: Vec<HuffmanNode> = Vec::with_capacity(2 * n);

    // Max-heap: (weight, serial, arena_index).
    // Weight closest to 0 pops first (matches Crystal's PriorityQueue.pop which takes
    // the highest-priority = highest weight = least-negative = closest to 0 item).
    // Serial breaks ties: higher serial (more recently pushed) pops first (LIFO).
    let mut heap: BinaryHeap<(i32, usize, usize)> = BinaryHeap::new();
    let mut serial: usize = 0;

    for i in 0..n {
        let w = -(i as i32);
        let idx = arena.len();
        arena.push(HuffmanNode { weight: w, children: vec![] });
        heap.push((w, serial, idx));
        serial += 1;
    }

    let initial_branches = initial_number_of_branches(n, arity);
    let mut first = true;

    while heap.len() > 1 {
        let n_branches = if first {
            first = false;
            initial_branches
        } else {
            arity
        };

        let take = n_branches.min(heap.len());
        let mut child_indices = Vec::with_capacity(take);
        let mut total_weight = 0i32;

        for _ in 0..take {
            let (w, _, idx) = heap.pop().unwrap();
            child_indices.push(idx);
            total_weight += w;
        }

        let new_idx = arena.len();
        arena.push(HuffmanNode { weight: total_weight, children: child_indices });
        heap.push((total_weight, serial, new_idx));
        serial += 1;
    }

    let root_idx = heap.pop().map(|(_, _, idx)| idx).unwrap_or(0);

    let mut result = Vec::with_capacity(n);
    traverse(&arena, root_idx, &mut Vec::new(), alphabet, &mut result);
    result.sort_by_key(|s: &String| s.len());

    save_cache(alphabet, n, &result);
    result
}

fn traverse(
    arena: &[HuffmanNode],
    idx: usize,
    path: &mut Vec<usize>,
    alphabet: &[String],
    out: &mut Vec<String>,
) {
    let node = &arena[idx];
    if node.children.is_empty() {
        let hint: String = path.iter().map(|&i| alphabet[i].as_str()).collect();
        out.push(hint);
    } else {
        for (i, &child_idx) in node.children.iter().enumerate() {
            path.push(i);
            traverse(arena, child_idx, path, alphabet, out);
            path.pop();
        }
    }
}

fn initial_number_of_branches(n: usize, arity: usize) -> usize {
    let mut result = 1usize;
    for t in 1..=(n / arity + 1) {
        let candidate = n as isize - t as isize * (arity as isize - 1);
        if candidate >= 2 && candidate as usize <= arity {
            result = candidate as usize;
            break;
        }
        result = arity;
    }
    result
}

fn cache_key(alphabet: &[String], n: usize) -> std::path::PathBuf {
    let key = format!("{}-{}", alphabet.concat(), n);
    cache_dir().join(key)
}

fn read_cache(alphabet: &[String], n: usize) -> Option<Vec<String>> {
    let data = std::fs::read_to_string(cache_key(alphabet, n)).ok()?;
    let items: Vec<String> = data.trim().split(':').map(str::to_string).collect();
    if items.is_empty() || (items.len() == 1 && items[0].is_empty()) {
        None
    } else {
        Some(items)
    }
}

fn save_cache(alphabet: &[String], n: usize, hints: &[String]) {
    let path = cache_key(alphabet, n);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(&path, hints.join(":"));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn alpha(s: &str) -> Vec<String> {
        s.chars().map(|c| c.to_string()).collect()
    }

    #[test]
    fn hints_at_most_alphabet_size_are_single_chars() {
        let ab = alpha("asdf");
        let hints = generate_hints(&ab, 4);
        assert_eq!(hints.len(), 4);
        for h in &hints {
            assert_eq!(h.len(), 1);
        }
    }

    #[test]
    fn hint_count_matches_n_and_all_unique() {
        let ab = alpha("asd");
        for n in 1..=12 {
            let hints = generate_hints(&ab, n);
            assert_eq!(hints.len(), n, "count mismatch at n={n}");
            let unique: std::collections::HashSet<_> = hints.iter().collect();
            assert_eq!(unique.len(), n, "duplicates at n={n}: {hints:?}");
        }
    }

    #[test]
    fn hints_sorted_by_length() {
        let ab = alpha("asdf");
        let hints = generate_hints(&ab, 10);
        for w in hints.windows(2) {
            assert!(w[0].len() <= w[1].len(), "not sorted: {w:?}");
        }
    }

    #[test]
    fn single_hint() {
        let ab = alpha("a");
        assert_eq!(generate_hints(&ab, 1), vec!["a"]);
    }

    #[test]
    fn two_char_alphabet_many_hints() {
        let ab = alpha("as");
        let n = 8;
        let hints = generate_hints(&ab, n);
        assert_eq!(hints.len(), n);
        let unique: std::collections::HashSet<_> = hints.iter().collect();
        assert_eq!(unique.len(), n);
    }
}

#[cfg(test)]
mod uuid_test {
    #[test]
    fn uuid_pattern_matches() {
        let re = regex::Regex::new(r"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}").unwrap();
        let line = "SHA: deadbeef1234567 UUID: 12345678-1234-1234-1234-123456789abc";
        let m: Vec<_> = re.find_iter(line).map(|m| (m.start(), m.as_str())).collect();
        println!("uuid matches: {m:?}");
        assert!(!m.is_empty(), "UUID pattern must match");
    }

    #[test]
    fn overlap_skip_longest_wins() {
        let uuid_re = regex::Regex::new(r"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}").unwrap();
        let sha_re  = regex::Regex::new(r"[0-9a-f]{7,128}").unwrap();
        let digit_re = regex::Regex::new(r"[0-9]{4,}").unwrap();
        let line = "UUID: 12345678-1234-1234-1234-123456789abc";

        // Intentionally add sha BEFORE uuid (simulating unfavorable HashMap ordering)
        let mut raw: Vec<(usize, usize)> = Vec::new();
        for cap in sha_re.find_iter(line)   { raw.push((cap.start(), cap.end())); }
        for cap in digit_re.find_iter(line) { raw.push((cap.start(), cap.end())); }
        for cap in uuid_re.find_iter(line)  { raw.push((cap.start(), cap.end())); }

        // Sort: same start → longest (largest end) first
        raw.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| b.1.cmp(&a.1)));

        let mut processed: Vec<(usize, usize)> = Vec::new();
        let mut last_end = 0;
        for (start, end) in raw {
            if start < last_end { continue; }
            processed.push((start, end));
            last_end = end;
        }
        println!("processed: {processed:?}");
        // UUID (start=6, end=42) should be the only match
        assert_eq!(processed.len(), 1, "UUID should beat sha+digit sub-matches: {processed:?}");
        assert_eq!(processed[0], (6, 42));
    }
}
