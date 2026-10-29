//! The node picker's list model: a profile's catalogue grouped by category,
//! folded, searched and filtered by a dragged wire. Headless, so it is tested
//! without a window; the GPUI picker only draws what this returns.

use std::collections::HashSet;

use graphy::DataType;
use graphy_render::Color;

use crate::profile::{CatalogEntry, DragMatch, GraphProfile};

/// One row of the picker list.
#[derive(Clone, Debug, PartialEq)]
pub enum Row {
    Header {
        name: String,
        color: Option<Color>,
        /// Nodes in the category.
        count: usize,
        /// Whether the category's entries are showing.
        open: bool,
        /// While a search narrows the list: how many nodes of the category match.
        matched: Option<usize>,
    },
    /// An index into the entry list the rows were built from.
    Entry(usize),
}

fn matches(entry: &CatalogEntry, query: &str) -> bool {
    entry.title.to_lowercase().contains(query)
        || entry.description.to_lowercase().contains(query)
        || entry.keywords.iter().any(|k| k.to_lowercase().contains(query))
}

/// `(category, indices of its entries)` in order of first appearance.
fn groups(entries: &[CatalogEntry]) -> Vec<(&str, Vec<usize>)> {
    let mut out: Vec<(&str, Vec<usize>)> = Vec::new();
    for (i, entry) in entries.iter().enumerate() {
        match out.iter_mut().find(|(name, _)| *name == entry.category) {
            Some((_, members)) => members.push(i),
            None => out.push((entry.category.as_str(), vec![i])),
        }
    }
    out
}

/// The entries that can take a wire dragged from an output of type `from`, each
/// category listing exact fits before wildcard-only ones.
pub fn filter_for_drag(
    profile: &dyn GraphProfile,
    entries: Vec<CatalogEntry>,
    from: &DataType,
) -> Vec<CatalogEntry> {
    let mut matched: Vec<(usize, DragMatch)> = entries
        .iter()
        .enumerate()
        .filter_map(|(i, e)| profile.drag_match(e, from).map(|m| (i, m)))
        .collect();
    // Stable: keeps the profile's order inside each class.
    matched.sort_by_key(|(_, m)| *m == DragMatch::Wildcard);
    let order: Vec<usize> = matched.iter().map(|(i, _)| *i).collect();
    let mut slots: Vec<Option<CatalogEntry>> = entries.into_iter().map(Some).collect();
    order.into_iter().filter_map(|i| slots[i].take()).collect()
}

/// The rows to show.
///
/// Categories are folded by default. While `query` is not empty a category with
/// matches opens by itself and shows only its matches; one without is hidden. A
/// category in `expanded` (opened by hand) shows all its entries, as long as it
/// has a match when searching. `open_all` opens every category (used when the
/// list is already narrowed to what fits a dragged wire).
pub fn visible_rows(
    entries: &[CatalogEntry],
    query: &str,
    expanded: &HashSet<String>,
    open_all: bool,
) -> Vec<Row> {
    let query = query.trim().to_lowercase();
    let searching = !query.is_empty();
    let mut rows = Vec::new();
    for (name, members) in groups(entries) {
        let hits: Vec<usize> = if searching {
            members
                .iter()
                .copied()
                .filter(|&i| matches(&entries[i], &query))
                .collect()
        } else {
            members.clone()
        };
        if searching && hits.is_empty() {
            continue;
        }
        let manual = expanded.contains(name);
        let open = manual || searching || open_all;
        rows.push(Row::Header {
            name: name.to_string(),
            color: members.iter().find_map(|&i| entries[i].color),
            count: members.len(),
            open,
            matched: searching.then_some(hits.len()),
        });
        if open {
            let shown = if manual { &members } else { &hits };
            rows.extend(shown.iter().map(|&i| Row::Entry(i)));
        }
    }
    rows
}

/// How many entries match `query` (all when it is empty), however folded.
pub fn matching_count(entries: &[CatalogEntry], query: &str) -> usize {
    let query = query.trim().to_lowercase();
    entries
        .iter()
        .filter(|e| query.is_empty() || matches(e, &query))
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use graphy::{NodeInstance, Position};
    use graphy_render::Presenter;

    fn entry(def: &str, category: &str, title: &str, description: &str) -> CatalogEntry {
        CatalogEntry {
            definition: def.into(),
            title: title.into(),
            category: category.into(),
            description: description.into(),
            ..Default::default()
        }
    }

    fn library() -> Vec<CatalogEntry> {
        vec![
            entry("add", "Math", "Add", "Sum of two numbers"),
            entry("sub", "Math", "Subtract", "Difference"),
            entry("print", "Debug", "Print String", "Writes to the log"),
            entry("branch", "Flow", "Branch", "If / else"),
        ]
    }

    fn shape(rows: &[Row], entries: &[CatalogEntry]) -> Vec<String> {
        rows.iter()
            .map(|r| match r {
                Row::Header { name, open, .. } => {
                    format!("{}{name}", if *open { "v " } else { "> " })
                }
                Row::Entry(i) => format!("  {}", entries[*i].definition),
            })
            .collect()
    }

    fn none() -> HashSet<String> {
        HashSet::new()
    }

    #[test]
    fn everything_starts_folded() {
        let l = library();
        assert_eq!(
            shape(&visible_rows(&l, "", &none(), false), &l),
            ["> Math", "> Debug", "> Flow"]
        );
    }

    #[test]
    fn a_category_the_user_opened_shows_all_its_nodes() {
        let l = library();
        let open: HashSet<String> = ["Math".to_string()].into();
        assert_eq!(
            shape(&visible_rows(&l, "", &open, false), &l),
            ["v Math", "  add", "  sub", "> Debug", "> Flow"]
        );
    }

    #[test]
    fn a_search_opens_matching_categories_to_their_matches_only() {
        let l = library();
        assert_eq!(
            shape(&visible_rows(&l, "sum", &none(), false), &l),
            ["v Math", "  add"]
        );
    }

    #[test]
    fn a_hand_opened_category_stays_complete_during_a_search_but_hides_without_a_match() {
        let l = library();
        let open: HashSet<String> = ["Math".to_string(), "Flow".to_string()].into();
        assert_eq!(
            shape(&visible_rows(&l, "sum", &open, false), &l),
            ["v Math", "  add", "  sub"]
        );
    }

    #[test]
    fn searching_ignores_case_and_surrounding_space_and_matches_keywords() {
        let mut l = library();
        l[3].keywords = vec!["conditional".into()];
        assert_eq!(visible_rows(&l, "  BRANCH ", &none(), false).len(), 2);
        assert_eq!(visible_rows(&l, "conditional", &none(), false).len(), 2);
        assert!(visible_rows(&l, "zzz", &none(), false).is_empty());
    }

    #[test]
    fn open_all_unfolds_everything_and_the_count_ignores_folding() {
        let l = library();
        assert_eq!(visible_rows(&l, "", &none(), true).len(), 7);
        assert_eq!(matching_count(&l, ""), 4);
        assert_eq!(matching_count(&l, "math"), 0);
        assert_eq!(matching_count(&l, "number"), 1);
    }

    /// A profile whose catalogue entries declare their input types.
    struct P;
    impl GraphProfile for P {
        fn id(&self) -> &str {
            "p"
        }
        fn presenter(&self) -> &dyn Presenter {
            unreachable!("not used by the filter")
        }
        fn create_node(&self, _: &str, _: &str, _: Position) -> Option<NodeInstance> {
            None
        }
    }

    #[test]
    fn dragging_a_wire_keeps_only_nodes_that_take_its_type() {
        let mut l = library();
        l[0].inputs = Some(vec![DataType::typed("f32")]);
        l[1].inputs = Some(vec![DataType::typed("String")]);
        l[2].inputs = Some(vec![DataType::Exec]);
        l[3].inputs = Some(vec![]);
        let kept = filter_for_drag(&P, l, &DataType::typed("f32"));
        let defs: Vec<_> = kept.iter().map(|e| e.definition.as_str()).collect();
        assert_eq!(defs, ["add"]);
    }
}
