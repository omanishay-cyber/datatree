//! BGE reranker (cross-encoder).
//!
//! Feature-gated behind `--features reranker`. When disabled (the default),
//! [`Reranker::new`] returns a no-op pass-through implementation and
//! [`Reranker::rerank`] returns its input unchanged. This keeps the whole
//! workspace buildable on tier-1 platforms without demanding a particular
//! `fastembed` feature set.
//!
//! The bge-reranker-v2-m3 model is NOT enabled in the default feature set
//! of this crate because `fastembed` pins different subsets of reranker
//! models across versions. Gating behind a feature flag makes the upgrade
//! path purely additive.

use crate::error::BrainResult;

/// Cross-encoder reranker. Cheap to clone; rerank is `&self`.
pub struct Reranker {
    #[cfg(feature = "reranker")]
    inner: RerankerInner,
}

impl std::fmt::Debug for Reranker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Reranker")
            .field("enabled", &cfg!(feature = "reranker"))
            .finish()
    }
}

impl Reranker {
    /// Construct a new reranker. When the feature flag is off this returns
    /// a pass-through stub without failing.
    pub fn new() -> BrainResult<Self> {
        #[cfg(feature = "reranker")]
        {
            let inner = RerankerInner::try_new()?;
            Ok(Self { inner })
        }
        #[cfg(not(feature = "reranker"))]
        {
            Ok(Self {})
        }
    }

    /// Rescore `candidates` relative to `query`.
    ///
    /// - If the feature is off, returns `candidates` unchanged.
    /// - If the model fails to load at runtime, returns `candidates`
    ///   unchanged. Reranking is strictly additive; failure must not
    ///   degrade retrieval.
    pub fn rerank(
        &self,
        query: &str,
        candidates: Vec<(String, f32)>,
    ) -> BrainResult<Vec<(String, f32)>> {
        #[cfg(feature = "reranker")]
        {
            self.inner.rerank(query, candidates)
        }
        #[cfg(not(feature = "reranker"))]
        {
            let _ = query;
            Ok(candidates)
        }
    }
}

// ---------------------------------------------------------------------------
// Gated inner impl
// ---------------------------------------------------------------------------

#[cfg(feature = "reranker")]
struct RerankerInner {
    // Wire up the concrete backend when fastembed's reranker surface
    // stabilises. Today this is a no-op shell that preserves input order —
    // it's enough to prove the plumbing compiles and the Engine hooks hit.
}

#[cfg(feature = "reranker")]
impl RerankerInner {
    fn try_new() -> BrainResult<Self> {
        Ok(Self {})
    }

    fn rerank(
        &self,
        _query: &str,
        candidates: Vec<(String, f32)>,
    ) -> BrainResult<Vec<(String, f32)>> {
        Ok(candidates)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passthrough_preserves_input() {
        let rr = Reranker::new().unwrap();
        let input = vec![("a".into(), 0.9), ("b".into(), 0.5)];
        let out = rr.rerank("q", input.clone()).unwrap();
        assert_eq!(out.len(), input.len());
    }
}
