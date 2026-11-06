// A name table that is cheap to copy, for type inference's environments.
//
// Inference copies its table of names on every block, branch and loop it looks into (to undo the names
// bound inside). With a plain HashMap each copy took the whole table, and on a 74-line program that was
// most of a 300 ms check. Here a copy shares the table (`base`) and keeps its own changes on top; the
// changes are folded into a fresh table once there are many of them. Reads and writes behave as a HashMap's.
use std::collections::HashMap;
use std::rc::Rc;

pub struct Map<V> {
    base: Rc<HashMap<String, V>>,
    // A name set (Some) or removed (None) since the table was shared.
    top: HashMap<String, Option<V>>,
}

const FOLD_AT: usize = 24;

impl<V: Clone> Map<V> {
    pub fn new() -> Self {
        Map { base: Rc::new(HashMap::new()), top: HashMap::new() }
    }

    pub fn get(&self, k: &str) -> Option<&V> {
        match self.top.get(k) {
            Some(v) => v.as_ref(),
            None => self.base.get(k),
        }
    }

    pub fn contains_key(&self, k: &str) -> bool {
        self.get(k).is_some()
    }

    pub fn insert(&mut self, k: String, v: V) -> Option<V> {
        if let Some(b) = Rc::get_mut(&mut self.base) {
            // Not shared: write in place.
            let old_top = self.top.remove(&k);
            let old = b.insert(k, v);
            return match old_top {
                Some(t) => t,
                None => old,
            };
        }
        let old = self.get(&k).cloned();
        self.top.insert(k, Some(v));
        self.fold();
        old
    }

    pub fn remove(&mut self, k: &str) -> Option<V> {
        let old = self.get(k).cloned();
        if let Some(b) = Rc::get_mut(&mut self.base) {
            self.top.remove(k);
            b.remove(k);
        } else if old.is_some() {
            if self.base.contains_key(k) {
                self.top.insert(k.to_string(), None);
            } else {
                self.top.remove(k);
            }
        }
        old
    }

    pub fn retain(&mut self, mut keep: impl FnMut(&String, &V) -> bool) {
        let gone: Vec<String> = self.iter().filter(|(k, v)| !keep(k, v)).map(|(k, _)| k.clone()).collect();
        for k in gone {
            self.remove(&k);
        }
    }

    pub fn len(&self) -> usize {
        self.iter().count()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &V)> {
        let top = self.top.iter().filter_map(|(k, v)| v.as_ref().map(|v| (k, v)));
        let base = self.base.iter().filter(|(k, _)| !self.top.contains_key(k.as_str()));
        top.chain(base)
    }

    pub fn keys(&self) -> impl Iterator<Item = &String> {
        self.iter().map(|(k, _)| k)
    }

    pub fn values(&self) -> impl Iterator<Item = &V> {
        self.iter().map(|(_, v)| v)
    }

    // Many changes on a shared table: make them a table of their own, so lookups stay one probe.
    fn fold(&mut self) {
        if self.top.len() < FOLD_AT {
            return;
        }
        let mut b: HashMap<String, V> = (*self.base).clone();
        for (k, v) in self.top.drain() {
            match v {
                Some(v) => {
                    b.insert(k, v);
                }
                None => {
                    b.remove(&k);
                }
            }
        }
        self.base = Rc::new(b);
    }
}

impl<V: Clone> Clone for Map<V> {
    fn clone(&self) -> Self {
        Map { base: Rc::clone(&self.base), top: self.top.clone() }
    }
}

impl<V: Clone> std::ops::Index<&str> for Map<V> {
    type Output = V;
    fn index(&self, k: &str) -> &V {
        self.get(k).expect("name in table")
    }
}

impl<V: Clone> std::ops::Index<&String> for Map<V> {
    type Output = V;
    fn index(&self, k: &String) -> &V {
        self.get(k).expect("name in table")
    }
}

impl<V: Clone> Default for Map<V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<V: Clone + std::fmt::Debug> std::fmt::Debug for Map<V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_map().entries(self.iter()).finish()
    }
}

impl<V: Clone + PartialEq> PartialEq for Map<V> {
    fn eq(&self, o: &Self) -> bool {
        self.len() == o.len() && self.iter().all(|(k, v)| o.get(k) == Some(v))
    }
}

impl<V: Clone> FromIterator<(String, V)> for Map<V> {
    fn from_iter<I: IntoIterator<Item = (String, V)>>(it: I) -> Self {
        Map { base: Rc::new(it.into_iter().collect()), top: HashMap::new() }
    }
}

impl<V: Clone> Extend<(String, V)> for Map<V> {
    fn extend<I: IntoIterator<Item = (String, V)>>(&mut self, it: I) {
        for (k, v) in it {
            self.insert(k, v);
        }
    }
}

impl<V: Clone> IntoIterator for Map<V> {
    type Item = (String, V);
    type IntoIter = std::vec::IntoIter<(String, V)>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter().map(|(k, v)| (k.clone(), v.clone())).collect::<Vec<_>>().into_iter()
    }
}

impl<'a, V: Clone> IntoIterator for &'a Map<V> {
    type Item = (&'a String, &'a V);
    type IntoIter = std::vec::IntoIter<(&'a String, &'a V)>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter().collect::<Vec<_>>().into_iter()
    }
}

#[cfg(test)]
mod tests {
    use super::Map;
    use std::collections::HashMap;

    #[test]
    fn behaves_as_a_hashmap() {
        // Random edits on a plain HashMap and on copies of a Map must agree at every step.
        let mut seed: u64 = 7;
        let mut rnd = |n: u64| {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (seed >> 33) % n
        };
        let mut plain: Vec<HashMap<String, u64>> = vec![HashMap::new()];
        let mut ours: Vec<Map<u64>> = vec![Map::new()];
        for step in 0..20000 {
            let i = rnd(plain.len() as u64) as usize;
            let k = format!("k{}", rnd(60));
            match rnd(10) {
                0 if plain.len() < 12 => {
                    plain.push(plain[i].clone());
                    ours.push(ours[i].clone());
                }
                1 if plain.len() > 1 => {
                    plain.remove(i);
                    ours.remove(i);
                }
                2 | 3 => assert_eq!(plain[i].remove(&k), ours[i].remove(&k), "step {step}"),
                _ => assert_eq!(plain[i].insert(k.clone(), step), ours[i].insert(k, step), "step {step}"),
            }
            for (p, o) in plain.iter().zip(&ours) {
                assert_eq!(p.len(), o.len(), "step {step}");
                for (k, v) in p {
                    assert_eq!(o.get(k), Some(v), "step {step}");
                }
            }
        }
    }
}
