use core::{
    array,
    borrow::Borrow,
    fmt,
    hash::{BuildHasher, Hash},
    mem::MaybeUninit,
};

pub mod hash {
    use core::hash::{BuildHasherDefault, Hasher};

    const FNV_OFFSET_BASIS: u64 = 0xCBF2_9CE4_8422_2325;
    const FNV_PRIME: u64 = 1_099_511_628_211;

    /// A simple FNV-1a hasher for small keys.
    pub struct FnvHasher(u64);

    impl Default for FnvHasher {
        fn default() -> Self {
            Self(FNV_OFFSET_BASIS)
        }
    }

    impl Hasher for FnvHasher {
        fn write(&mut self, bytes: &[u8]) {
            for byte in bytes {
                self.0 ^= u64::from(*byte);
                self.0 = self.0.wrapping_mul(FNV_PRIME);
            }
        }

        fn finish(&self) -> u64 {
            self.0
        }
    }

    /// Builds `FnvHasher` instances.
    pub type FnvBuildHasher = BuildHasherDefault<FnvHasher>;
}

const NONE: usize = usize::MAX;

/// A fixed-capacity cache entry returned by eviction and removal operations.
#[derive(Debug, PartialEq, Eq)]
pub struct Entry<K, V, M = ()> {
    pub key: K,
    pub value: V,
    pub meta: M,
}

impl<K, V, M> Entry<K, V, M> {
    #[must_use]
    #[inline]
    pub fn into_parts(self) -> (K, V, M) {
        (self.key, self.value, self.meta)
    }
}

/// Result of inserting an item into the cache.
#[derive(Debug, PartialEq, Eq)]
pub enum InsertResult<K, V, M = ()> {
    Added,
    Replaced { value: V, meta: M },
    Evicted(Entry<K, V, M>),
}

/// Read-only view into a cache entry.
#[derive(Clone, Copy, Debug)]
pub struct EntryRef<'a, K, V, M = ()> {
    key: &'a K,
    value: &'a V,
    meta: &'a M,
}

impl<'a, K, V, M> EntryRef<'a, K, V, M> {
    #[must_use]
    #[inline]
    pub const fn key(&self) -> &'a K {
        self.key
    }

    #[must_use]
    #[inline]
    pub const fn value(&self) -> &'a V {
        self.value
    }

    #[must_use]
    #[inline]
    pub const fn meta(&self) -> &'a M {
        self.meta
    }
}

/// Mutable view into a cache entry.
#[derive(Debug)]
pub struct EntryRefMut<'a, K, V, M = ()> {
    key: &'a K,
    value: &'a mut V,
    meta: &'a mut M,
}

impl<'a, K, V, M> EntryRefMut<'a, K, V, M> {
    #[must_use]
    #[inline]
    pub const fn key(&self) -> &K {
        self.key
    }

    #[must_use]
    #[inline]
    pub const fn value(&self) -> &V {
        &*self.value
    }

    #[must_use]
    #[inline]
    pub const fn value_mut(&mut self) -> &mut V {
        &mut *self.value
    }

    #[must_use]
    #[inline]
    pub const fn meta(&self) -> &M {
        &*self.meta
    }

    #[must_use]
    #[inline]
    pub const fn meta_mut(&mut self) -> &mut M {
        &mut *self.meta
    }

    #[must_use]
    #[inline]
    pub const fn into_parts(self) -> (&'a K, &'a mut V, &'a mut M) {
        (self.key, self.value, self.meta)
    }
}

/// Lookup index used by the LRU arena.
pub trait IndexStorage<const CAPACITY: usize> {
    fn clear(&mut self);

    fn find<F>(&self, hash: u64, is_match: F) -> Option<usize>
    where
        F: FnMut(usize) -> bool;

    fn insert(&mut self, hash: u64, index: usize);

    fn remove(&mut self, hash: u64, index: usize);

    fn bucket_count(&self) -> usize;
}

/// Allocation-free lookup index backed by fixed arrays.
#[derive(Debug)]
pub struct ArrayIndex<const CAPACITY: usize, const BUCKETS: usize> {
    heads: [usize; BUCKETS],
    next: [usize; CAPACITY],
}

impl<const CAPACITY: usize, const BUCKETS: usize> ArrayIndex<CAPACITY, BUCKETS> {
    #[must_use]
    /// Creates an empty index with the specified shape.
    ///
    /// # Panics
    ///
    /// Panics if `BUCKETS == 0`.
    pub const fn new() -> Self {
        assert!(BUCKETS > 0, "bucket count must be non-zero");

        Self {
            heads: [NONE; BUCKETS],
            next: [NONE; CAPACITY],
        }
    }

    #[expect(clippy::cast_possible_truncation)]
    const fn bucket(hash: u64) -> usize {
        (hash as usize) % BUCKETS
    }
}

impl<const CAPACITY: usize, const BUCKETS: usize> Default for ArrayIndex<CAPACITY, BUCKETS> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const CAPACITY: usize, const BUCKETS: usize> IndexStorage<CAPACITY>
    for ArrayIndex<CAPACITY, BUCKETS>
{
    fn clear(&mut self) {
        self.heads.fill(NONE);
        self.next.fill(NONE);
    }

    fn find<F>(&self, hash: u64, mut is_match: F) -> Option<usize>
    where
        F: FnMut(usize) -> bool,
    {
        let mut current = self.heads[Self::bucket(hash)];

        while current != NONE {
            if is_match(current) {
                return Some(current);
            }

            current = self.next[current];
        }

        None
    }

    fn insert(&mut self, hash: u64, index: usize) {
        debug_assert!(index < CAPACITY);

        let bucket = Self::bucket(hash);

        self.next[index] = self.heads[bucket];
        self.heads[bucket] = index;
    }

    fn remove(&mut self, hash: u64, index: usize) {
        debug_assert!(index < CAPACITY);

        let bucket = Self::bucket(hash);
        let mut current = self.heads[bucket];
        let mut previous = NONE;

        while current != NONE {
            if current == index {
                let next = self.next[current];

                if previous == NONE {
                    self.heads[bucket] = next;
                } else {
                    self.next[previous] = next;
                }

                self.next[current] = NONE;
                return;
            }

            previous = current;
            current = self.next[current];
        }

        debug_assert!(false, "index missing from lookup table");
    }

    fn bucket_count(&self) -> usize {
        BUCKETS
    }
}

#[derive(Debug)]
struct Slot<K, V, M> {
    payload: MaybeUninit<Entry<K, V, M>>,
    hash: u64,
    lru_prev: usize,
    lru_next: usize,
    free_next: usize,
    occupied: bool,
}

impl<K, V, M> Slot<K, V, M> {
    const fn vacant(next_free: usize) -> Self {
        Self {
            payload: MaybeUninit::uninit(),
            hash: 0,
            lru_prev: NONE,
            lru_next: NONE,
            free_next: next_free,
            occupied: false,
        }
    }
}

/// A fixed-capacity, allocation-free LRU cache.
///
/// The cache owns a preallocated slot array and delegates lookup to an index
/// storage implementation. Recency tracking uses an intrusive doubly-linked
/// list over the slots.
///
/// # Examples
///
/// ```rust
/// # use hyperdrive::queues::lru::{Entry, LruCache};
/// #
/// let mut cache = LruCache::<u32, u32, 4, 4>::default();
///
/// cache.insert(1, 10);
/// cache.insert(2, 20);
///
/// assert_eq!(cache.get(&1), Some(&10));
/// assert_eq!(
///     cache.pop_lru(),
///     Some(Entry {
///         key: 2,
///         value: 20,
///         meta: ()
///     })
/// );
/// ```
pub struct IndexedLruCache<
    K,
    V,
    const CAPACITY: usize,
    I: IndexStorage<CAPACITY>,
    S = hash::FnvBuildHasher,
    M = (),
> {
    slots: [Slot<K, V, M>; CAPACITY],
    index: I,
    free_head: usize,
    lru_head: usize,
    lru_tail: usize,
    len: usize,
    hash_builder: S,
}

pub type LruCache<
    K,
    V,
    const CAPACITY: usize,
    const BUCKETS: usize,
    S = hash::FnvBuildHasher,
    M = (),
> = IndexedLruCache<K, V, CAPACITY, ArrayIndex<CAPACITY, BUCKETS>, S, M>;

impl<K, V, const CAPACITY: usize, I, S, M> IndexedLruCache<K, V, CAPACITY, I, S, M>
where
    I: IndexStorage<CAPACITY>,
{
    /// Creates a cache with a caller-provided lookup index and hasher builder.
    ///
    /// # Panics
    ///
    /// Panics if `CAPACITY == 0`.
    #[must_use]
    pub fn with_index_and_hasher(index: I, hash_builder: S) -> Self {
        assert!(CAPACITY > 0, "cache capacity must be non-zero");

        let slots = array::from_fn(|index| {
            let next = if index + 1 == CAPACITY {
                NONE
            } else {
                index + 1
            };
            Slot::vacant(next)
        });

        Self {
            slots,
            index,
            free_head: 0,
            lru_head: NONE,
            lru_tail: NONE,
            len: 0,
            hash_builder,
        }
    }

    /// Creates a cache with a caller-provided hasher builder.
    ///
    /// # Panics
    ///
    /// Panics if `CAPACITY == 0`, or if the default index rejects its shape.
    #[must_use]
    pub fn with_hasher(hash_builder: S) -> Self
    where
        I: Default,
    {
        Self::with_index_and_hasher(I::default(), hash_builder)
    }

    #[must_use]
    #[inline]
    pub const fn len(&self) -> usize {
        self.len
    }

    #[must_use]
    #[inline]
    pub const fn capacity(&self) -> usize {
        CAPACITY
    }

    #[must_use]
    #[inline]
    pub fn bucket_count(&self) -> usize
    where
        I: IndexStorage<CAPACITY>,
    {
        self.index.bucket_count()
    }

    #[must_use]
    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[must_use]
    #[inline]
    pub const fn is_full(&self) -> bool {
        self.len == CAPACITY
    }

    pub fn clear(&mut self)
    where
        I: IndexStorage<CAPACITY>,
    {
        while let Some(entry) = self.pop_lru() {
            drop(entry);
        }
        self.index.clear();
    }

    #[must_use]
    pub fn contains_key<Q>(&self, key: &Q) -> bool
    where
        K: Borrow<Q>,
        Q: Eq + Hash + ?Sized,
        S: BuildHasher,
        I: IndexStorage<CAPACITY>,
    {
        let hash = self.hash_key(key);
        self.find_index(key, hash).is_some()
    }

    #[must_use]
    #[inline]
    pub fn peek<Q>(&self, key: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: Eq + Hash + ?Sized,
        S: BuildHasher,
        I: IndexStorage<CAPACITY>,
    {
        self.peek_entry(key).map(|entry| entry.value())
    }

    #[inline]
    pub fn peek_mut<Q>(&mut self, key: &Q) -> Option<&mut V>
    where
        K: Borrow<Q>,
        Q: Eq + Hash + ?Sized,
        S: BuildHasher,
        I: IndexStorage<CAPACITY>,
    {
        self.peek_entry_mut(key).map(|entry| entry.into_parts().1)
    }

    #[must_use]
    pub fn peek_entry<Q>(&self, key: &Q) -> Option<EntryRef<'_, K, V, M>>
    where
        K: Borrow<Q>,
        Q: Eq + Hash + ?Sized,
        S: BuildHasher,
        I: IndexStorage<CAPACITY>,
    {
        let hash = self.hash_key(key);
        let index = self.find_index(key, hash)?;
        Some(unsafe { self.entry_ref(index) })
    }

    pub fn peek_entry_mut<Q>(&mut self, key: &Q) -> Option<EntryRefMut<'_, K, V, M>>
    where
        K: Borrow<Q>,
        Q: Eq + Hash + ?Sized,
        S: BuildHasher,
        I: IndexStorage<CAPACITY>,
    {
        let hash = self.hash_key(key);
        let index = self.find_index(key, hash)?;
        Some(unsafe { self.entry_ref_mut(index) })
    }

    #[must_use]
    pub fn get<Q>(&mut self, key: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: Eq + Hash + ?Sized,
        S: BuildHasher,
        I: IndexStorage<CAPACITY>,
    {
        let value_mut = self.get_mut(key)?;
        Some(value_mut)
    }

    pub fn get_mut<Q>(&mut self, key: &Q) -> Option<&mut V>
    where
        K: Borrow<Q>,
        Q: Eq + Hash + ?Sized,
        S: BuildHasher,
        I: IndexStorage<CAPACITY>,
    {
        let hash = self.hash_key(key);
        let index = self.find_index(key, hash)?;
        self.touch(index);
        Some(unsafe { self.value_mut(index) })
    }

    #[must_use]
    pub fn get_entry<Q>(&mut self, key: &Q) -> Option<EntryRef<'_, K, V, M>>
    where
        K: Borrow<Q>,
        Q: Eq + Hash + ?Sized,
        S: BuildHasher,
        I: IndexStorage<CAPACITY>,
    {
        let hash = self.hash_key(key);
        let index = self.find_index(key, hash)?;
        self.touch(index);
        Some(unsafe { self.entry_ref(index) })
    }

    pub fn get_entry_mut<Q>(&mut self, key: &Q) -> Option<EntryRefMut<'_, K, V, M>>
    where
        K: Borrow<Q>,
        Q: Eq + Hash + ?Sized,
        S: BuildHasher,
        I: IndexStorage<CAPACITY>,
    {
        let hash = self.hash_key(key);
        let index = self.find_index(key, hash)?;
        self.touch(index);
        Some(unsafe { self.entry_ref_mut(index) })
    }

    /// Inserts or replaces an entry, returning any displaced payload.
    ///
    /// # Panics
    ///
    /// Panics only if the internal occupancy bookkeeping has been corrupted
    /// and a full cache no longer contains an evictable entry.
    pub fn insert(&mut self, key: K, value: V) -> InsertResult<K, V, M>
    where
        K: Eq + Hash,
        S: BuildHasher,
        M: Default,
        I: IndexStorage<CAPACITY>,
    {
        self.insert_with_meta(key, value, M::default())
    }

    /// Inserts or replaces an entry, returning any displaced payload.
    ///
    /// # Panics
    ///
    /// Panics only if the internal occupancy bookkeeping has been corrupted
    /// and a full cache no longer contains an evictable entry.
    pub fn insert_with_meta(&mut self, key: K, value: V, meta: M) -> InsertResult<K, V, M>
    where
        K: Eq + Hash,
        S: BuildHasher,
        I: IndexStorage<CAPACITY>,
    {
        let hash = self.hash_key(&key);

        if let Some(index) = self.find_index(&key, hash) {
            let entry = unsafe { self.slots[index].payload.assume_init_mut() };
            let old_value = core::mem::replace(&mut entry.value, value);
            let old_meta = core::mem::replace(&mut entry.meta, meta);

            self.touch(index);

            return InsertResult::Replaced {
                value: old_value,
                meta: old_meta,
            };
        }

        let evicted = if self.len == CAPACITY {
            let entry = self
                .pop_lru()
                .expect("full cache must evict one least-recently-used entry");
            Some(entry)
        } else {
            None
        };

        let index = self.alloc_slot();

        {
            let slot = &mut self.slots[index];
            slot.payload.write(Entry { key, value, meta });
            slot.hash = hash;
            slot.lru_prev = NONE;
            slot.lru_next = NONE;
            slot.occupied = true;
        }

        self.index.insert(hash, index);
        self.push_front(index);
        self.len += 1;

        evicted.map_or(InsertResult::Added, InsertResult::Evicted)
    }

    pub fn remove<Q>(&mut self, key: &Q) -> Option<Entry<K, V, M>>
    where
        K: Borrow<Q>,
        Q: Eq + Hash + ?Sized,
        S: BuildHasher,
        I: IndexStorage<CAPACITY>,
    {
        let hash = self.hash_key(key);
        let index = self.find_index(key, hash)?;
        Some(self.remove_at(index))
    }

    pub fn pop_lru(&mut self) -> Option<Entry<K, V, M>>
    where
        I: IndexStorage<CAPACITY>,
    {
        (self.lru_tail != NONE).then(|| self.remove_at(self.lru_tail))
    }

    pub fn pop_mru(&mut self) -> Option<Entry<K, V, M>>
    where
        I: IndexStorage<CAPACITY>,
    {
        (self.lru_head != NONE).then(|| self.remove_at(self.lru_head))
    }

    pub fn retain<F>(&mut self, mut keep: F)
    where
        F: FnMut(EntryRefMut<K, V, M>) -> bool,
        I: IndexStorage<CAPACITY>,
    {
        for index in 0..CAPACITY {
            if !self.slots[index].occupied {
                continue;
            }
            let entry = unsafe { self.entry_ref_mut(index) };
            if !keep(entry) {
                let _ = self.remove_at(index);
            }
        }
    }

    fn hash_key<Q>(&self, key: &Q) -> u64
    where
        Q: Hash + ?Sized,
        S: BuildHasher,
    {
        self.hash_builder.hash_one(key)
    }

    fn find_index<Q>(&self, key: &Q, hash: u64) -> Option<usize>
    where
        K: Borrow<Q>,
        Q: Eq + ?Sized,
        I: IndexStorage<CAPACITY>,
    {
        self.index.find(hash, |index| {
            let slot = &self.slots[index];

            slot.hash == hash && unsafe { self.key_ref(index) }.borrow() == key
        })
    }

    fn alloc_slot(&mut self) -> usize {
        let index = self.free_head;
        debug_assert!(index != NONE, "cache slot allocation without free capacity");
        self.free_head = self.slots[index].free_next;
        self.slots[index].free_next = NONE;
        index
    }

    const fn recycle_slot(&mut self, index: usize) {
        let slot = &mut self.slots[index];
        slot.hash = 0;
        slot.lru_prev = NONE;
        slot.lru_next = NONE;
        slot.occupied = false;
        slot.free_next = self.free_head;
        self.free_head = index;
    }

    const fn push_front(&mut self, index: usize) {
        let old_head = self.lru_head;
        self.lru_head = index;

        {
            let slot = &mut self.slots[index];
            slot.lru_prev = NONE;
            slot.lru_next = old_head;
        }

        if old_head == NONE {
            self.lru_tail = index;
        } else {
            self.slots[old_head].lru_prev = index;
        }
    }

    const fn unlink(&mut self, index: usize) {
        let prev = self.slots[index].lru_prev;
        let next = self.slots[index].lru_next;

        if prev == NONE {
            self.lru_head = next;
        } else {
            self.slots[prev].lru_next = next;
        }

        if next == NONE {
            self.lru_tail = prev;
        } else {
            self.slots[next].lru_prev = prev;
        }

        self.slots[index].lru_prev = NONE;
        self.slots[index].lru_next = NONE;
    }

    const fn touch(&mut self, index: usize) {
        if self.lru_head == index {
            return;
        }

        self.unlink(index);
        self.push_front(index);
    }

    fn remove_at(&mut self, index: usize) -> Entry<K, V, M>
    where
        I: IndexStorage<CAPACITY>,
    {
        debug_assert!(self.slots[index].occupied);

        let hash = self.slots[index].hash;

        self.index.remove(hash, index);
        self.unlink(index);
        self.len -= 1;

        let entry = unsafe { self.slots[index].payload.assume_init_read() };

        self.recycle_slot(index);
        entry
    }

    // # Safety
    //
    // The caller must ensure the slot at `index` is occupied.
    unsafe fn key_ref(&self, index: usize) -> &K {
        let slot = &self.slots[index];
        debug_assert!(slot.occupied);

        let entry = unsafe { slot.payload.assume_init_ref() };
        &entry.key
    }

    // # Safety
    //
    // The caller must ensure the slot at `index` is occupied.
    unsafe fn value_mut(&mut self, index: usize) -> &mut V {
        let slot = &mut self.slots[index];
        debug_assert!(slot.occupied);

        let entry = unsafe { slot.payload.assume_init_mut() };
        &mut entry.value
    }

    // # Safety
    //
    // The caller must ensure the slot at `index` is occupied.
    unsafe fn entry_ref(&self, index: usize) -> EntryRef<'_, K, V, M> {
        let slot = &self.slots[index];
        debug_assert!(slot.occupied);

        let entry = unsafe { slot.payload.assume_init_ref() };

        EntryRef {
            key: &entry.key,
            value: &entry.value,
            meta: &entry.meta,
        }
    }

    // # Safety
    //
    // The caller must ensure the slot at `index` is occupied.
    unsafe fn entry_ref_mut(&mut self, index: usize) -> EntryRefMut<'_, K, V, M> {
        let slot = &mut self.slots[index];
        debug_assert!(slot.occupied);

        let entry = unsafe { slot.payload.assume_init_mut() };

        EntryRefMut {
            key: &entry.key,
            value: &mut entry.value,
            meta: &mut entry.meta,
        }
    }
}

impl<K, V, const CAPACITY: usize, I, S, M> Drop for IndexedLruCache<K, V, CAPACITY, I, S, M>
where
    I: IndexStorage<CAPACITY>,
{
    fn drop(&mut self) {
        self.clear();
    }
}

impl<K, V, const CAPACITY: usize, I, S, M> Default for IndexedLruCache<K, V, CAPACITY, I, S, M>
where
    I: Default + IndexStorage<CAPACITY>,
    S: Default,
{
    fn default() -> Self {
        Self::with_hasher(S::default())
    }
}

impl<K, V, const CAPACITY: usize, I, S, M> fmt::Debug for IndexedLruCache<K, V, CAPACITY, I, S, M>
where
    K: fmt::Debug,
    V: fmt::Debug,
    M: fmt::Debug,
    I: IndexStorage<CAPACITY>,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        struct EntriesDebug<'a, K, V, const CAPACITY: usize, I: IndexStorage<CAPACITY>, S, M>(
            &'a IndexedLruCache<K, V, CAPACITY, I, S, M>,
        );

        impl<K, V, const CAPACITY: usize, I, S, M> fmt::Debug for EntriesDebug<'_, K, V, CAPACITY, I, S, M>
        where
            K: fmt::Debug,
            V: fmt::Debug,
            M: fmt::Debug,
            I: IndexStorage<CAPACITY>,
        {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                let mut list = f.debug_list();
                let mut current = self.0.lru_head;

                while current != NONE {
                    let slot = &self.0.slots[current];
                    let entry = unsafe { self.0.entry_ref(current) };
                    let item = (entry.key(), entry.value(), entry.meta());
                    list.entry(&item);
                    current = slot.lru_next;
                }

                list.finish()
            }
        }

        f.debug_struct("LruCache")
            .field("len", &self.len)
            .field("capacity", &CAPACITY)
            .field("entries_mru_first", &EntriesDebug(self))
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::{Entry, InsertResult, LruCache, hash};

    type Cache = LruCache<u32, u32, 4, 4, hash::FnvBuildHasher>;
    type MetaCache = LruCache<u32, u32, 4, 8, hash::FnvBuildHasher, u64>;

    #[test]
    fn insert_and_get_updates_recency() {
        let mut cache = Cache::default();

        assert_eq!(cache.insert(1, 10), InsertResult::Added);
        assert_eq!(cache.insert(2, 20), InsertResult::Added);
        assert_eq!(cache.insert(3, 30), InsertResult::Added);

        assert_eq!(cache.get(&1), Some(&10));
        assert_eq!(
            cache.pop_lru(),
            Some(Entry {
                key: 2,
                value: 20,
                meta: ()
            })
        );
        assert_eq!(
            cache.pop_lru(),
            Some(Entry {
                key: 3,
                value: 30,
                meta: ()
            })
        );
        assert_eq!(
            cache.pop_lru(),
            Some(Entry {
                key: 1,
                value: 10,
                meta: ()
            })
        );
    }

    #[test]
    fn insert_evicts_tail_when_full() {
        let mut cache = Cache::default();

        for key in 0..4 {
            assert_eq!(cache.insert(key, key * 10), InsertResult::Added);
        }

        assert_eq!(cache.get(&2), Some(&20));

        assert_eq!(
            cache.insert(4, 40),
            InsertResult::Evicted(Entry {
                key: 0,
                value: 0,
                meta: (),
            })
        );

        assert!(!cache.contains_key(&0));
        assert!(cache.contains_key(&2));
        assert!(cache.contains_key(&4));
    }

    #[test]
    fn replacing_entry_keeps_slot_and_returns_old_payload() {
        let mut cache = MetaCache::default();

        assert_eq!(cache.insert_with_meta(7, 70, 100), InsertResult::Added);
        assert_eq!(
            cache.insert_with_meta(7, 71, 101),
            InsertResult::Replaced {
                value: 70,
                meta: 100,
            }
        );

        let entry = cache.peek_entry(&7).expect("replaced entry must exist");
        assert_eq!((*entry.key(), *entry.value(), *entry.meta()), (7, 71, 101));
    }

    #[test]
    fn removal_returns_owned_entry() {
        let mut cache = MetaCache::default();

        assert_eq!(cache.insert_with_meta(1, 10, 11), InsertResult::Added);
        assert_eq!(
            cache.remove(&1),
            Some(Entry {
                key: 1,
                value: 10,
                meta: 11,
            })
        );
        assert!(cache.is_empty());
    }

    #[test]
    fn retain_can_prune_entries_in_place() {
        let mut cache = MetaCache::default();

        for key in 0..4 {
            assert_eq!(
                cache.insert_with_meta(key, key * 10, u64::from(key)),
                InsertResult::Added
            );
        }

        cache.retain(|entry| {
            let (_key, value, meta) = entry.into_parts();
            *value += 1;
            *meta += 10;
            *meta >= 12
        });

        assert!(!cache.contains_key(&0));
        assert!(!cache.contains_key(&1));

        let two = cache.peek_entry(&2).expect("key 2 should remain");
        assert_eq!((*two.value(), *two.meta()), (21, 12));

        let three = cache.peek_entry(&3).expect("key 3 should remain");
        assert_eq!((*three.value(), *three.meta()), (31, 13));
    }
}
