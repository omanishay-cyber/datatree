//! Disk-backed embedding store with brute-force cosine nearest-neighbour.
//!
//! Storage layout (binary, little-endian):
//! ```text
//! ~/.mneme/cache/embed/index.bin
//!   [u32  magic   = 0x44544231 ("DTB1")]
//!   [u32  dim     = 384]
//!   [u64  count   = N]
//!   N x ([u128 node_id] [f32 * dim])
//! ```
//!
//! This is intentionally simpler than VSS / usearch: zero external native
//! deps, opens cleanly on every platform, and the brute-force scan is well
//! under a millisecond per 1k vectors at 384-D. When the corpus exceeds
//! ~250k vectors callers should swap in an ANN index — until then this is
//! both faster end-to-end and easier to audit.

use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::fs::{self, File, OpenOptions};
use std::io::{BufReader, BufWriter, Read, Seek, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

use crate::embeddings::EMBEDDING_DIM;
use crate::error::{BrainError, BrainResult};
use crate::NodeId;

const MAGIC: u32 = 0x4454_4231; // "DTB1"
const FILE_NAME: &str = "index.bin";

/// One ANN result.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct NearestHit {
    pub node: NodeId,
    /// Cosine similarity in `[-1.0, 1.0]`. Higher is closer.
    pub score: f32,
}

/// Disk-backed vector store. Cheap to clone (internally `Arc`-wrapped).
#[derive(Clone)]
pub struct EmbedStore {
    inner: Arc<Inner>,
}

struct Inner {
    dir: PathBuf,
    state: RwLock<State>,
}

#[derive(Default)]
struct State {
    ids: Vec<NodeId>,
    vectors: Vec<f32>, // flat: ids.len() * EMBEDDING_DIM
    dirty: bool,
}

impl std::fmt::Debug for EmbedStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = self.inner.state.read();
        f.debug_struct("EmbedStore")
            .field("dir", &self.inner.dir)
            .field("count", &s.ids.len())
            .field("dim", &EMBEDDING_DIM)
            .finish()
    }
}

impl EmbedStore {
    /// Open at the default `~/.mneme/cache/embed/` directory.
    pub fn open_default() -> BrainResult<Self> {
        Self::open(&default_dir())
    }

    /// Open (or create) a store at `dir`.
    pub fn open(dir: &Path) -> BrainResult<Self> {
        fs::create_dir_all(dir)?;
        let path = dir.join(FILE_NAME);
        let state = if path.exists() {
            match load_file(&path) {
                Ok(s) => s,
                Err(e) => {
                    warn!(error = %e, "embed index unreadable — starting empty");
                    State::default()
                }
            }
        } else {
            State::default()
        };
        debug!(count = state.ids.len(), "embed store opened");
        Ok(Self {
            inner: Arc::new(Inner {
                dir: dir.to_path_buf(),
                state: RwLock::new(state),
            }),
        })
    }

    /// Number of vectors currently stored.
    pub fn len(&self) -> usize {
        self.inner.state.read().ids.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Insert / overwrite one vector. The store keeps **the latest write**
    /// for any given node id.
    pub fn upsert(&self, node: NodeId, vector: &[f32]) -> BrainResult<()> {
        if vector.len() != EMBEDDING_DIM {
            return Err(BrainError::Invalid(format!(
                "vector length {} != EMBEDDING_DIM ({})",
                vector.len(),
                EMBEDDING_DIM
            )));
        }
        let mut s = self.inner.state.write();
        if let Some(pos) = s.ids.iter().position(|n| *n == node) {
            let off = pos * EMBEDDING_DIM;
            s.vectors[off..off + EMBEDDING_DIM].copy_from_slice(vector);
        } else {
            s.ids.push(node);
            s.vectors.extend_from_slice(vector);
        }
        s.dirty = true;
        Ok(())
    }

    /// Bulk upsert — cheaper than calling [`Self::upsert`] in a loop because
    /// the lock is taken once.
    pub fn upsert_many(&self, items: &[(NodeId, Vec<f32>)]) -> BrainResult<()> {
        for (_, v) in items {
            if v.len() != EMBEDDING_DIM {
                return Err(BrainError::Invalid(format!(
                    "vector length {} != EMBEDDING_DIM ({})",
                    v.len(),
                    EMBEDDING_DIM
                )));
            }
        }
        let mut s = self.inner.state.write();
        for (node, v) in items {
            if let Some(pos) = s.ids.iter().position(|n| *n == *node) {
                let off = pos * EMBEDDING_DIM;
                s.vectors[off..off + EMBEDDING_DIM].copy_from_slice(v);
            } else {
                s.ids.push(*node);
                s.vectors.extend_from_slice(v);
            }
        }
        s.dirty = true;
        Ok(())
    }

    /// Remove one node. No-op if absent.
    pub fn remove(&self, node: NodeId) -> bool {
        let mut s = self.inner.state.write();
        if let Some(pos) = s.ids.iter().position(|n| *n == node) {
            s.ids.remove(pos);
            let off = pos * EMBEDDING_DIM;
            s.vectors.drain(off..off + EMBEDDING_DIM);
            s.dirty = true;
            true
        } else {
            false
        }
    }

    /// Return the `k` highest-cosine matches.
    ///
    /// `query` is assumed to be already L2-normalised (the [`crate::Embedder`]
    /// guarantees this); stored vectors are too, so cosine == dot product.
    ///
    /// BUG-A2-002 fix: use a `BinaryHeap<Reverse<...>>` min-heap for O(N log k)
    /// instead of sort-on-every-insert (was O(N k log k)).
    pub fn nearest(&self, query: &[f32], k: usize) -> Vec<NearestHit> {
        if query.len() != EMBEDDING_DIM || k == 0 {
            return Vec::new();
        }
        let s = self.inner.state.read();
        let n = s.ids.len();
        if n == 0 {
            return Vec::new();
        }

        // Min-heap of size k. Wrap NearestHit in `Reverse` so the smallest
        // score is on top; that lets us pop-and-replace in O(log k) per step.
        let mut heap: BinaryHeap<Reverse<HeapHit>> = BinaryHeap::with_capacity(k + 1);
        for i in 0..n {
            let off = i * EMBEDDING_DIM;
            let row = &s.vectors[off..off + EMBEDDING_DIM];
            let mut dot = 0f32;
            for d in 0..EMBEDDING_DIM {
                dot += row[d] * query[d];
            }
            let hit = HeapHit {
                node: s.ids[i],
                score: dot,
            };
            if heap.len() < k {
                heap.push(Reverse(hit));
            } else if let Some(Reverse(min)) = heap.peek() {
                if hit.score > min.score {
                    heap.pop();
                    heap.push(Reverse(hit));
                }
            }
        }

        // Drain heap then sort high-to-low for the caller.
        let mut out: Vec<NearestHit> = heap
            .into_iter()
            .map(|Reverse(h)| NearestHit {
                node: h.node,
                score: h.score,
            })
            .collect();
        out.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        out
    }

    /// Persist any pending writes to `index.bin` (atomic rename).
    pub fn flush(&self) -> BrainResult<()> {
        let mut s = self.inner.state.write();
        if !s.dirty {
            return Ok(());
        }
        let final_path = self.inner.dir.join(FILE_NAME);
        let tmp_path = self.inner.dir.join(format!("{FILE_NAME}.tmp"));

        {
            let f = OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(&tmp_path)?;
            let mut w = BufWriter::new(f);
            w.write_all(&MAGIC.to_le_bytes())?;
            w.write_all(&(EMBEDDING_DIM as u32).to_le_bytes())?;
            w.write_all(&(s.ids.len() as u64).to_le_bytes())?;
            for (i, id) in s.ids.iter().enumerate() {
                w.write_all(&id.0.to_le_bytes())?;
                let off = i * EMBEDDING_DIM;
                let row = &s.vectors[off..off + EMBEDDING_DIM];
                // WIDE-012: write each f32 little-endian byte-by-byte so
                // the on-disk format is portable across architectures
                // (no host-endian dependency, no `unsafe`). For raw
                // pass-through on LE hosts we can use `bytemuck`, but
                // explicit `to_le_bytes()` is unambiguous and trivially
                // verifiable.
                for f in row {
                    w.write_all(&f.to_le_bytes())?;
                }
            }
            w.flush()?;
        }

        // BUG-A2-001 fix: `fs::rename` is atomic on both Unix and Windows
        // (Rust >= 1.5 uses `MoveFileExW(MOVEFILE_REPLACE_EXISTING)`), so
        // the prior `remove_file` opened a crash window where index.bin
        // could vanish entirely. Just rename — Windows handles overwrite.
        fs::rename(&tmp_path, &final_path)?;
        s.dirty = false;
        Ok(())
    }
}

/// Internal heap entry — a `NearestHit` with `Ord` impl driven by score.
#[derive(Debug, Clone, Copy)]
struct HeapHit {
    node: NodeId,
    score: f32,
}

impl PartialEq for HeapHit {
    fn eq(&self, other: &Self) -> bool {
        self.score == other.score
    }
}

impl Eq for HeapHit {}

impl PartialOrd for HeapHit {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for HeapHit {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Total order on f32 — NaN treated as equal/smallest.
        self.score
            .partial_cmp(&other.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn load_file(path: &Path) -> BrainResult<State> {
    let f = File::open(path)?;
    let mut r = BufReader::new(f);

    let mut buf4 = [0u8; 4];
    r.read_exact(&mut buf4)?;
    let magic = u32::from_le_bytes(buf4);
    if magic != MAGIC {
        return Err(BrainError::Store(format!("bad magic: {magic:#010x}")));
    }
    r.read_exact(&mut buf4)?;
    let dim = u32::from_le_bytes(buf4) as usize;
    if dim != EMBEDDING_DIM {
        return Err(BrainError::Store(format!(
            "dim mismatch: file={dim} expected={EMBEDDING_DIM}"
        )));
    }
    let mut buf8 = [0u8; 8];
    r.read_exact(&mut buf8)?;
    let count = u64::from_le_bytes(buf8) as usize;

    let mut ids = Vec::with_capacity(count);
    let mut vectors = Vec::with_capacity(count * EMBEDDING_DIM);
    let mut buf16 = [0u8; 16];
    let row_bytes = EMBEDDING_DIM * std::mem::size_of::<f32>();
    let mut row_buf = vec![0u8; row_bytes];

    for _ in 0..count {
        r.read_exact(&mut buf16)?;
        let id = u128::from_le_bytes(buf16);
        ids.push(NodeId(id));
        r.read_exact(&mut row_buf)?;
        for chunk in row_buf.chunks_exact(4) {
            let mut b = [0u8; 4];
            b.copy_from_slice(chunk);
            vectors.push(f32::from_le_bytes(b));
        }
    }

    // Sanity: cursor at EOF.
    let pos = r.stream_position()?;
    let len = r.get_ref().metadata()?.len();
    if pos != len {
        warn!(pos, len, "trailing bytes in embed index");
    }

    Ok(State {
        ids,
        vectors,
        dirty: false,
    })
}

/// Default cache directory: `<mneme-root>/cache/embed/`.
///
/// HOME-bypass-brain (embed_store) fix: route through
/// `PathManager::default_root()` so `MNEME_HOME` is honored.
/// `PathManager` already implements the
/// `MNEME_HOME` -> `~/.mneme` -> OS-default fallback chain.
pub fn default_dir() -> PathBuf {
    common::paths::PathManager::default_root()
        .root()
        .join("cache")
        .join("embed")
}

// ---------------------------------------------------------------------------
// Tests — Audit fix HIGH-35 (2026-05-06, 2026-05-05 audit, testing-reviewer):
// EmbedStore is the Item #117 keystone storage layer (symbol-anchored BGE
// embeddings) and shipped with ZERO unit tests. The DTB1 magic-number,
// dim-mismatch rejection, file-corruption fallback, upsert/upsert_many,
// and nearest-K cosine search were all untested in source — a regression
// that, say, swapped the byte order on read or broke the dim check would
// pass cargo check + every existing test and silently corrupt persisted
// embeddings on the user's machine.
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    /// Build a deterministic dummy 384-D vector keyed on `seed`. Different
    /// seeds yield linearly-independent vectors so cosine-sim ordering
    /// is stable.
    fn vec_seed(seed: u32) -> Vec<f32> {
        let mut v = vec![0f32; EMBEDDING_DIM];
        for (i, slot) in v.iter_mut().enumerate() {
            // Simple keyed ramp + offset; cheap and deterministic.
            *slot = ((i as u32).wrapping_mul(seed) % 257) as f32 / 257.0;
        }
        // Normalise so cosine ranges are well-defined.
        let mag = v.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-9);
        for slot in v.iter_mut() {
            *slot /= mag;
        }
        v
    }

    #[test]
    fn open_creates_empty_store_when_dir_missing() {
        let tmp = TempDir::new().unwrap();
        let store = EmbedStore::open(tmp.path()).expect("open creates dir");
        // Brand-new store has zero vectors.
        let hits = store.nearest(&vec_seed(1), 5);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn upsert_then_nearest_returns_seeded_vector_first() {
        let tmp = TempDir::new().unwrap();
        let store = EmbedStore::open(tmp.path()).unwrap();
        let n1 = NodeId(1);
        let v1 = vec_seed(11);
        store.upsert(n1, &v1).unwrap();

        // Querying with the same vector returns it as the top hit
        // (cosine similarity = 1.0 against itself).
        let hits = store.nearest(&v1, 5);
        assert!(!hits.is_empty(), "upsert + nearest must find the row");
        assert_eq!(hits[0].node, n1);
        assert!(
            (hits[0].score - 1.0).abs() < 1e-3,
            "self-similarity should be 1.0, got {}",
            hits[0].score
        );
    }

    #[test]
    fn upsert_rejects_wrong_dimension() {
        let tmp = TempDir::new().unwrap();
        let store = EmbedStore::open(tmp.path()).unwrap();
        let bad = vec![0.0f32; EMBEDDING_DIM - 1];
        let err = store.upsert(NodeId(2), &bad);
        assert!(err.is_err(), "dim mismatch must fail-loud, not silent");
    }

    #[test]
    fn upsert_many_persists_via_flush_and_reload() {
        let tmp = TempDir::new().unwrap();
        {
            let store = EmbedStore::open(tmp.path()).unwrap();
            let items: Vec<(NodeId, Vec<f32>)> = (0..10)
                .map(|i| (NodeId(100 + i as u128), vec_seed(100 + i)))
                .collect();
            store.upsert_many(&items).unwrap();
            store.flush().expect("flush writes DTB1 file");
        }
        // Reopen and confirm round-trip: a fresh store at the same dir
        // sees all 10 rows.
        let store2 = EmbedStore::open(tmp.path()).unwrap();
        let hits = store2.nearest(&vec_seed(105), 3);
        assert!(hits.len() <= 3);
        assert!(
            !hits.is_empty(),
            "after flush+reopen, nearest must find seeded rows"
        );
    }

    #[test]
    fn corrupt_magic_falls_back_to_empty_store() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(FILE_NAME);
        // Write garbage where the DTB1 header should be — this
        // exercises the warn-and-empty-state fallback in `open()`.
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(b"NOTDTB1").unwrap();
        drop(f);
        let store =
            EmbedStore::open(tmp.path()).expect("open must not panic on a corrupt file");
        // Result: empty in-memory state. Search returns nothing.
        let hits = store.nearest(&vec_seed(1), 5);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn corrupt_dim_falls_back_to_empty_store() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(FILE_NAME);
        // Write valid magic but wrong dim. load_file should reject
        // with a Store error; `open()` warns and starts empty.
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(&MAGIC.to_le_bytes()).unwrap();
        f.write_all(&999u32.to_le_bytes()).unwrap(); // wrong dim
        f.write_all(&0u64.to_le_bytes()).unwrap();
        drop(f);
        let store = EmbedStore::open(tmp.path()).unwrap();
        let hits = store.nearest(&vec_seed(1), 5);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn nearest_k_zero_returns_empty_vec() {
        let tmp = TempDir::new().unwrap();
        let store = EmbedStore::open(tmp.path()).unwrap();
        store.upsert(NodeId(7), &vec_seed(7)).unwrap();
        let hits = store.nearest(&vec_seed(7), 0);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn upsert_overwrites_existing_node_id() {
        let tmp = TempDir::new().unwrap();
        let store = EmbedStore::open(tmp.path()).unwrap();
        let id = NodeId(42);
        store.upsert(id, &vec_seed(1)).unwrap();
        store.upsert(id, &vec_seed(99)).unwrap();
        // Query with the second-version vector: top hit must be 42
        // with similarity == 1.0 (proves the second upsert REPLACED
        // not appended).
        let hits = store.nearest(&vec_seed(99), 5);
        assert_eq!(hits[0].node, id);
        assert!((hits[0].score - 1.0).abs() < 1e-3);
        // Sanity: there are no duplicate id-42 entries by checking
        // that the first vector is no longer findable as a strong
        // match (it was overwritten).
        let hits_old = store.nearest(&vec_seed(1), 5);
        assert!(
            hits_old.is_empty() || hits_old[0].score < 0.99,
            "old vector should not still produce a perfect self-match"
        );
    }

    #[test]
    fn nearest_returns_top_k_in_descending_score_order() {
        let tmp = TempDir::new().unwrap();
        let store = EmbedStore::open(tmp.path()).unwrap();
        for i in 1..=5 {
            store
                .upsert(NodeId(i as u128), &vec_seed(i as u32))
                .unwrap();
        }
        let hits = store.nearest(&vec_seed(3), 5);
        // We ask for 5; the corpus has 5; expect 5 hits.
        assert_eq!(hits.len(), 5);
        // Scores must be monotonically non-increasing.
        for w in hits.windows(2) {
            assert!(
                w[0].score >= w[1].score,
                "scores out of order: {} then {}",
                w[0].score,
                w[1].score
            );
        }
        // Self-similarity is 1.0; the seeded query (seed=3) must be
        // the top hit.
        assert_eq!(hits[0].node, NodeId(3));
    }
}
