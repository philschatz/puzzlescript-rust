use std::fmt;

const BUCKET_SIZE: u16 = 128;
const BUCKET_COUNT: usize = 4;

#[derive(Copy, Clone, PartialEq, Eq, Debug, Hash)]
pub struct BitSet {
    _is_empty: bool,
    bits: [u128; BUCKET_COUNT],
}

impl BitSet {
    pub fn new() -> Self {
        Self { 
            _is_empty: true, 
            bits: [0; BUCKET_COUNT] 
        }
    }

    pub fn contains(&self, index: u16) -> bool {
        let bucket = index / BUCKET_SIZE;
        let rem = index % BUCKET_SIZE;
        self.bits[bucket as usize] & (1 << rem) > 0
    }
    pub fn insert(&mut self, index: u16) {
        let bucket = index / BUCKET_SIZE;
        let rem = index % BUCKET_SIZE;
        self.bits[bucket as usize] |= 1 << rem;
        self._is_empty = false
    }
    pub fn remove(&mut self, index: u16) {
        let bucket = index / BUCKET_SIZE;
        let rem = index % BUCKET_SIZE;
        self.bits[bucket as usize] &= !(1 << rem);
        self.update_emptiness();
    }

    pub fn contains_all(&self, other: &BitSet) -> bool {
        for bucket in 0..BUCKET_COUNT {
            if self.bits[bucket] & other.bits[bucket] != other.bits[bucket] {
                return false
            }
        }
        true
    }
    pub fn contains_none(&self, other: &BitSet) -> bool {
        // if other.is_empty() { return true }
        for bucket in 0..BUCKET_COUNT {
            if self.bits[bucket] & other.bits[bucket] != 0 {
                return false
            }
        }
        true
    }
    pub fn contains_any(&self, other: &BitSet) -> bool {
        if other.is_empty() { return true }

        for bucket in 0..BUCKET_COUNT {
            if self.bits[bucket] & other.bits[bucket] != 0 {
                return true
            }
        }
        false
    }
    pub fn insert_all(&mut self, other: &BitSet) {
        for bucket in 0..BUCKET_COUNT {
            self.bits[bucket] |= other.bits[bucket]
        }
    }
    fn remove_all(&mut self, other: &BitSet) {
        for bucket in 0..BUCKET_COUNT {
            self.bits[bucket] &= other.bits[bucket]
        }
    }

    fn is_empty(&self) -> bool {
        self._is_empty
    }

    fn update_emptiness(&mut self) {
        self._is_empty = true;
        for bucket in 0..BUCKET_COUNT {
            if self.bits[bucket] != 0 { 
                self._is_empty = false;
                break
            }
        }
    }

    // The number of true bits in the set
    fn cardinality(&self) -> u16 {
        let mut count = 0;
        for bucket in 0..BUCKET_COUNT {
            let mut i = self.bits[bucket];
            while i > 0 {
                if i % 2 == 1 {
                    count += 1;
                }
                i >>= 1;
            }
        }
        count
    }

    pub fn len(&self) -> u16 {
        self.cardinality()
    }

    pub fn into_vec(&self) -> Vec<u16> {
        let mut ret = vec![];
        let mut index = 0;

        for bucket in 0..BUCKET_COUNT {
            let mut i = self.bits[bucket];
            if i == 0 { index += BUCKET_SIZE; continue }
            while i > 0 {
                if i % 2 == 1 {
                    ret.push(index);
                }
                i >>= 1;
                index += 1;
            }
        }
        ret
    }
}

impl fmt::Display for BitSet {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "BitSet({:?})", self.into_vec())
    }
}



#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cardinality() {
        let mut bitset = BitSet::new();
        assert_eq!(bitset.cardinality(), 0);
        
        bitset.insert(1);
        assert_eq!(bitset.cardinality(), 1);

        bitset.insert(3);
        assert_eq!(bitset.cardinality(), 2);

        bitset.insert(127);
        assert_eq!(bitset.cardinality(), 3);
    }

    #[test]
    fn bitset() {
        let mut bitset = BitSet::new();
        bitset.insert(1);

        assert!(!bitset.contains(0));
        assert!(bitset.contains(1));
        
        bitset.insert(0);
        assert!(bitset.contains(0));
        
        bitset.remove(1);
        assert!(!bitset.contains(1));
        assert!(bitset.bits[0] == 1); // look inside the guts and verify
        
        bitset.insert(127);
        assert!(bitset.contains(127));
        bitset.remove(127);

        bitset.remove(0);
        bitset.insert(1);
        bitset.insert(2);
    }

    #[test]
    fn large() {
        let mut bitset = BitSet::new();
        bitset.insert(0);
        bitset.insert(127);
        assert_eq!(bitset.cardinality(), 2);
        bitset.insert(BUCKET_SIZE);
        assert_eq!(bitset.cardinality(), 3);
        bitset.insert(129);
        assert_eq!(bitset.cardinality(), 4);
        bitset.insert(303);
        assert_eq!(bitset.cardinality(), 5);
        assert!(bitset.contains(303));

        let mut bs = BitSet::new();
        bs.insert(303);
        assert!(bitset.contains_all(&bs));

        assert!(bitset.contains(129));
    }

    #[test]
    fn contains_any() {
        let mut bitset = BitSet::new();
        bitset.insert(0);
        bitset.insert(299);

        let mut bs2 = BitSet::new();

        assert!(bitset.contains_any(&bs2), "When the argument is empty then bitset contains any of those items (none)");
        assert!(bitset.contains_none(&bs2), "When the argument is empty then bitset definitely contains none of the items");

        bs2.insert(0);
        assert!(bitset.contains_any(&bs2));
        assert!(!bitset.contains_none(&bs2));

        bs2.insert(1);
        assert!(bitset.contains_any(&bs2));
        assert!(!bitset.contains_none(&bs2));

        bs2.remove(0);
        assert!(!bitset.contains_any(&bs2));
        assert!(bitset.contains_none(&bs2));
    }
}