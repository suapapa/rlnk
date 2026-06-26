//! Hash generation for short URLs.

use std::num::NonZeroUsize;

use rand::{Rng, distr::Alphanumeric};

/// Stateless generator for URL-safe short hashes.
#[derive(Clone, Debug)]
pub struct HashGenerator {
    length: NonZeroUsize,
}

impl HashGenerator {
    pub fn new(length: NonZeroUsize) -> Self {
        Self { length }
    }

    pub fn generate(&self) -> String {
        rand::rng()
            .sample_iter(Alphanumeric)
            .take(self.length.get())
            .map(char::from)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroUsize;

    use super::HashGenerator;

    #[test]
    fn generate_should_return_requested_length() {
        let generator = HashGenerator::new(NonZeroUsize::new(8).expect("8 is non-zero"));

        let value = generator.generate();

        assert_eq!(value.len(), 8);
    }

    #[test]
    fn generate_should_only_use_url_safe_alphanumeric_bytes() {
        let generator = HashGenerator::new(NonZeroUsize::new(16).expect("16 is non-zero"));

        let value = generator.generate();

        assert!(
            value
                .chars()
                .all(|character| character.is_ascii_alphanumeric())
        );
    }
}
