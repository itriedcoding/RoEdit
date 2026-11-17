//! Stable identifiers. Every id is a plain integer newtype so that documents
//! serialise deterministically and references stay cheap.

use serde::{Deserialize, Serialize};
use std::fmt;

macro_rules! id_type {
    ($name:ident, $inner:ty, $label:literal) => {
        #[derive(
            Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize,
        )]
        #[serde(transparent)]
        pub struct $name(pub $inner);

        impl $name {
            pub const fn raw(self) -> $inner {
                self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, concat!($label, "{}"), self.0)
            }
        }
    };
}

id_type!(EntityId, u64, "e");
id_type!(MeshId, u32, "mesh");
id_type!(MaterialId, u32, "mat");
id_type!(ImageId, u32, "img");
id_type!(LightId, u32, "light");
id_type!(TrackId, u32, "track");
id_type!(LayerId, u32, "layer");
id_type!(ClipId, u32, "clip");
id_type!(VideoTrackId, u32, "vtrack");

/// Monotonic counter helper. `next` always returns a fresh value.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Counter(pub u64);

impl Counter {
    // A monotonic id source, deliberately not an `Iterator`: it never ends.
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> u64 {
        let v = self.0;
        self.0 += 1;
        v
    }

    pub fn peek(&self) -> u64 {
        self.0
    }

    pub fn bump_to(&mut self, v: u64) {
        self.0 = self.0.max(v + 1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counters_are_monotonic() {
        let mut c = Counter::default();
        assert_eq!(c.next(), 0);
        assert_eq!(c.next(), 1);
        c.bump_to(10);
        assert_eq!(c.next(), 11);
    }

    #[test]
    fn ids_roundtrip_through_json() {
        let id = EntityId(42);
        let s = serde_json::to_string(&id).unwrap();
        assert_eq!(s, "42");
        assert_eq!(serde_json::from_str::<EntityId>(&s).unwrap(), id);
    }
}
