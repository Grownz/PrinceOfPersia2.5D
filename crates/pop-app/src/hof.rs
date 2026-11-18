//! Hall of fame (M6), after the original's (docs/cutscenes.md 4.10, VERIFIED in PRINCE.EXE): at most 6 entries of
//! (name, rem_min, rem_tick), most time left first, a new time goes below equal ones; name up to 24 characters; shown as
//! "M:SS" = rem_min - 1 : rem_tick / 12. The original keeps a binary PRINCE.HOF; ours is JSON `PRINCE.HOF` next to the exe,
//! written atomically, and a damaged file loads as far as it can (bad entries dropped, a copy kept as PRINCE.HOF.bad).
use bevy::prelude::Resource;
use serde::{Deserialize, Serialize};

/// end_sequence `0000:3036`: `cmp [bp-10h], 6` (6 slots of 29 bytes at ds:59D8)
pub const MAX: usize = 6;
/// name input `0C83:0544` with max length 0x18 (`0000:3150`)
pub const NAME_MAX: usize = 24;
/// what a controller player (no keyboard) gets
pub const DEFAULT_NAME: &str = "PRINCE";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub name: String,
    pub rem_min: i32,
    pub rem_tick: i32,
}

impl Entry {
    /// show_hof `0000:3406`: sprintf("%d:%02d", rem_min - 1, rem_tick / 12)
    pub fn time_text(&self) -> String {
        format!("{}:{:02}", self.rem_min - 1, self.rem_tick / 12)
    }

    fn valid(&self) -> bool {
        (1..=60).contains(&self.rem_min) && (0..=719).contains(&self.rem_tick) && self.name.chars().count() <= NAME_MAX
    }
}

#[derive(Resource, Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct Hof {
    pub entries: Vec<Entry>,
}

impl Hof {
    /// The slot a finish with this much time left gets (`0000:3008..303F`), or None when the list is full of better times.
    pub fn slot(&self, rem_min: i32, rem_tick: i32) -> Option<usize> {
        let i = self.entries.iter().position(|e| (e.rem_min, e.rem_tick) < (rem_min, rem_tick)).unwrap_or(self.entries.len());
        (i < MAX).then_some(i)
    }

    /// Insert at `slot` (the last entry drops off a full list); returns the slot.
    pub fn insert(&mut self, name: &str, rem_min: i32, rem_tick: i32) -> Option<usize> {
        let i = self.slot(rem_min, rem_tick)?;
        self.entries.insert(i, Entry { name: clip(name), rem_min, rem_tick });
        self.entries.truncate(MAX);
        Some(i)
    }

    /// Tolerant reader: a non-JSON file is an empty list; bad entries are dropped; the rest is sorted and capped.
    pub fn parse(text: &str) -> Hof {
        let Ok(serde_json::Value::Object(o)) = serde_json::from_str::<serde_json::Value>(text) else { return Hof::default() };
        let mut entries: Vec<Entry> = o
            .get("entries")
            .and_then(|v| v.as_array())
            .map(|a| a.iter().filter_map(|e| serde_json::from_value::<Entry>(e.clone()).ok()).filter(Entry::valid).collect())
            .unwrap_or_default();
        entries.sort_by_key(|e| std::cmp::Reverse((e.rem_min, e.rem_tick))); // stable: equal times keep their order
        entries.truncate(MAX);
        Hof { entries }
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("hof serialize")
    }

    pub fn path() -> std::path::PathBuf {
        crate::flow::exe_dir().join("PRINCE.HOF")
    }

    pub fn load() -> Hof {
        let Ok(t) = std::fs::read_to_string(Self::path()) else { return Hof::default() };
        if !matches!(serde_json::from_str(&t), Ok(serde_json::Value::Object(_))) {
            let _ = std::fs::write(Self::path().with_extension("HOF.bad"), &t); // the next save would overwrite it (best effort)
        }
        Hof::parse(&t)
    }

    pub fn save(&self) {
        if let Err(e) = crate::flow::write_atomic(&Self::path(), &self.to_json()) {
            bevy::log::warn!("cannot write PRINCE.HOF: {e}");
        }
    }
}

/// At most NAME_MAX characters, control characters dropped.
pub fn clip(name: &str) -> String {
    name.chars().filter(|c| !c.is_control()).take(NAME_MAX).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hof(times: &[(i32, i32)]) -> Hof {
        Hof { entries: times.iter().enumerate().map(|(i, &(m, t))| Entry { name: format!("N{i}"), rem_min: m, rem_tick: t }).collect() }
    }

    #[test]
    fn insert_sorts_by_time_left_and_caps_at_six() {
        let mut h = Hof::default();
        assert_eq!(h.insert("A", 20, 100), Some(0));
        assert_eq!(h.insert("B", 40, 0), Some(0));
        assert_eq!(h.insert("C", 20, 300), Some(1));
        // equal time: the new one goes below (jae at 0000:3034)
        assert_eq!(h.insert("D", 20, 300), Some(2));
        let names: Vec<&str> = h.entries.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["B", "C", "D", "A"]);
        assert_eq!(h.insert("E", 5, 1), Some(4));
        assert_eq!(h.insert("F", 50, 1), Some(0));
        assert_eq!(h.entries.len(), MAX);
        // full: worse than the last is refused, better pushes the last one out
        assert_eq!((h.slot(5, 1), h.insert("G", 1, 0)), (None, None));
        assert_eq!(h.insert("H", 30, 0), Some(2));
        assert_eq!((h.entries.len(), h.entries.last().unwrap().name.as_str()), (MAX, "A"));
    }

    #[test]
    fn names_are_clipped_and_time_text_matches_show_hof() {
        let mut h = Hof::default();
        h.insert(&"x".repeat(40), 60, 719);
        assert_eq!(h.entries[0].name.len(), NAME_MAX);
        assert_eq!(h.entries[0].time_text(), "59:59");
        assert_eq!(Entry { name: String::new(), rem_min: 1, rem_tick: 5 }.time_text(), "0:00");
        assert_eq!(clip("a\nb\tc"), "abc");
    }

    #[test]
    fn parse_round_trip_and_tolerance() {
        let h = hof(&[(50, 1), (30, 700), (30, 2)]);
        assert_eq!(Hof::parse(&h.to_json()), h);
        for bad in ["", "not json", "[1,2]", "{\"entries\": 3}", "{"] {
            assert_eq!(Hof::parse(bad), Hof::default(), "{bad:?}");
        }
        // bad entries dropped, the rest sorted and capped
        let t = r#"{"entries": [
            {"name": "low", "rem_min": 2, "rem_tick": 0},
            {"name": "bad min", "rem_min": 99, "rem_tick": 0},
            {"name": 5, "rem_min": 10, "rem_tick": 0},
            {"name": "top", "rem_min": 59, "rem_tick": 1},
            {"rem_min": 10},
            {"name": "a", "rem_min": 9, "rem_tick": 0}, {"name": "b", "rem_min": 8, "rem_tick": 0},
            {"name": "c", "rem_min": 7, "rem_tick": 0}, {"name": "d", "rem_min": 6, "rem_tick": 0}
        ], "future": true}"#;
        let names: Vec<String> = Hof::parse(t).entries.into_iter().map(|e| e.name).collect();
        assert_eq!(names, ["top", "a", "b", "c", "d", "low"]);
    }
}
