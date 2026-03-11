/// Compact bit set for tracking entity changes.
#[derive(Clone, Debug, Default)]
pub struct BitSet {
    bits: Vec<u64>,
}

impl BitSet {
    pub fn new() -> Self {
        Self { bits: Vec::new() }
    }

    pub fn set(&mut self, index: u32) {
        let word = index as usize / 64;
        let bit = index as usize % 64;
        if word >= self.bits.len() {
            self.bits.resize(word + 1, 0);
        }
        self.bits[word] |= 1 << bit;
    }

    pub fn unset(&mut self, index: u32) {
        let word = index as usize / 64;
        let bit = index as usize % 64;
        if word < self.bits.len() {
            self.bits[word] &= !(1 << bit);
        }
    }

    pub fn get(&self, index: u32) -> bool {
        let word = index as usize / 64;
        let bit = index as usize % 64;
        word < self.bits.len() && (self.bits[word] & (1 << bit)) != 0
    }

    pub fn clear(&mut self) {
        for word in &mut self.bits {
            *word = 0;
        }
    }

    pub fn iter_set(&self) -> BitSetIter<'_> {
        BitSetIter {
            bits: &self.bits,
            word_index: 0,
            current_word: self.bits.first().copied().unwrap_or(0),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.bits.iter().all(|&w| w == 0)
    }
}

pub struct BitSetIter<'a> {
    bits: &'a [u64],
    word_index: usize,
    current_word: u64,
}

impl Iterator for BitSetIter<'_> {
    type Item = u32;

    fn next(&mut self) -> Option<u32> {
        while self.current_word == 0 {
            self.word_index += 1;
            if self.word_index >= self.bits.len() {
                return None;
            }
            self.current_word = self.bits[self.word_index];
        }
        let bit = self.current_word.trailing_zeros();
        self.current_word &= self.current_word - 1; // clear lowest set bit
        Some(self.word_index as u32 * 64 + bit)
    }
}
