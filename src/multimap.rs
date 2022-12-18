use std::collections::HashMap;
use std::hash::Hash;

pub struct MultiMap<K, V> {
    next_index: usize,
    indexes: HashMap<K, usize>,
    values: HashMap<usize, V>,
}

impl<K, V> MultiMap<K, V> {
    pub fn iter(&self) -> impl Iterator<Item = &V> {
        self.values.values()
    }
}

impl<K, V> MultiMap<K, V>
where
    K: Hash + Eq,
{
    pub fn insert<I>(&mut self, keys: I, value: V)
    where
        I: IntoIterator<Item = K>,
    {
        let index = self.next_index;
        self.next_index += 1;

        self.values.insert(index, value);
        for k in keys.into_iter() {
            self.indexes.insert(k, index);
        }
    }

    pub fn get_mut(&mut self, key: &K) -> Option<&mut V> {
        self.indexes
            .get(key)
            .and_then(|idx| self.values.get_mut(idx))
    }
}

impl<K, V> Default for MultiMap<K, V> {
    fn default() -> Self {
        Self {
            next_index: 0,
            indexes: HashMap::new(),
            values: HashMap::new(),
        }
    }
}

impl<I, K, V> FromIterator<(I, V)> for MultiMap<K, V>
where
    I: IntoIterator<Item = K>,
    K: Hash + Eq,
{
    fn from_iter<T>(iter: T) -> Self
    where
        T: IntoIterator<Item = (I, V)>,
    {
        let mut map = Self::default();
        for (keys, value) in iter {
            map.insert(keys, value);
        }
        map
    }
}
