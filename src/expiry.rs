use std::{
    borrow::Borrow,
    collections::HashSet,
    hash::{Hash, Hasher},
    time::Instant,
};

#[derive(Eq)]
struct ExpireItem<T> {
    item: T,
    expiry: Instant,
}

impl<T> PartialEq for ExpireItem<T>
where
    T: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.item.eq(&other.item)
    }
}

impl<T> Hash for ExpireItem<T>
where
    T: Hash,
{
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.item.hash(state);
    }
}

impl<T> Borrow<T> for ExpireItem<T> {
    fn borrow(&self) -> &T {
        &self.item
    }
}

#[derive(Default)]
pub struct ExpireSet<T> {
    inner: HashSet<ExpireItem<T>>,
}

impl<T> ExpireSet<T> {
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.inner.iter().map(|i| &i.item)
    }
}

impl<T> ExpireSet<T>
where
    T: Eq + Hash,
{
    pub fn insert(&mut self, item: T, expiry: Instant) {
        let expiry = if let Some(item) = self.inner.get(&item) {
            if expiry > item.expiry {
                expiry
            } else {
                item.expiry
            }
        } else {
            expiry
        };

        self.inner.replace(ExpireItem { item, expiry });
    }

    pub fn extend_from(&mut self, other: ExpireSet<T>) {
        for item in other.inner {
            self.insert(item.item, item.expiry);
        }
    }

    pub fn expire(&mut self) {
        let now = Instant::now();
        let mut newset: HashSet<ExpireItem<T>> = HashSet::new();
        newset.reserve(self.inner.len());
        newset.extend(self.inner.drain().filter(|ei| ei.expiry < now));
        self.inner = newset;
    }
}
