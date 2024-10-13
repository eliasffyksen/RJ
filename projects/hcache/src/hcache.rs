use std::hash::{Hash, Hasher};

use ahash::{AHasher, AHashMap};

type HashT = u64;

fn hash<T: Hash>(t: &T) -> HashT {
    let mut hasher = AHasher::default();
    t.hash(&mut hasher);
    hasher.finish()
}

#[derive(Default, Debug)]
pub struct Cache<V>
{
    cache: AHashMap<HashT, V>,
}

impl<V> Cache<V>
{
    pub fn new() -> Self {
        Cache {
            cache: AHashMap::new(),
        }
    }

    pub fn get<K: Hash>(&mut self, underlying: fn(&K) -> V, key: &K) -> &V {
        let hash = hash(key);

        if self.cache.contains_key(&hash) {
            return self.cache.get(&hash).unwrap();
        }

        let new_value = underlying(key);
        self.cache.insert(hash, new_value);
        self.cache.get(&hash).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_construction() {
        Cache::<i64>::new();
    }

    #[test]
    fn test_cache_double_get() {
        let mut cache = Cache::new();

        // Check that the cache is working
        let result = cache.get(|&x| x * 2, &5);
        assert_eq!(*result, 10);

        // Check that underlying is not called again
        let result = cache.get(|&_x| panic!("Value not cached!"), &5);
        assert_eq!(*result, 10);
    }

    #[test]
    fn test_cache_multiple_values() {
        let mut cache = Cache::new();

        // Check that the cache is working
        let result = cache.get(|&x| x * 2, &5);
        assert_eq!(*result, 10);

        // Check that the cache is working
        let result = cache.get(|&x| x * 2, &6);
        assert_eq!(*result, 12);

        // Check that underlying is not called again
        let result = cache.get(|&_x| panic!("Value not cached!"), &5);
        assert_eq!(*result, 10);

        // Check that underlying is not called again
        let result = cache.get(|&_x| panic!("Value not cached!"), &6);
        assert_eq!(*result, 12);
    }
}

