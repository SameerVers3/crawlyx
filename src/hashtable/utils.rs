use ahash::AHasher;
use std::hash::{Hash, Hasher};

pub const NUM_SHARDS: usize = 64; // I need to make it configureable (we can only make it power of 2 for
                           // making a perfect bitmask for the sharding index)
pub const SHARD_MASK: usize = NUM_SHARDS - 1;

// checking if the shard if power of 2 (because obviously it will be driven from config)

const _: () = assert!(
    NUM_SHARDS.is_power_of_two(),
    "Shard should be a power of two."
);

pub fn hash_key<K: Hash + ?Sized>(key: &K) -> u64 {
    let mut hasher = AHasher::default();
    key.hash(&mut hasher);
    hasher.finish()
}

// this maps the value to a shard index liek [0..63]
// bitmasking to find the hash -> faster the modulo
pub fn shard_index(hash: u64) -> usize {
    (hash as usize) & SHARD_MASK
}

// hash a key and return the shard shard_index
pub fn shard_for<K: Hash + ?Sized>(key: &K) -> usize {
    shard_index(hash_key(key))
}
