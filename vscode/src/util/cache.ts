/**
 * Tiny time-bounded LRU cache, shared across providers.
 *
 * Two use sites:
 *   - per-file blast radius cache keyed by fsPath + content hash
 *   - per-query recall cache to avoid re-running the same recall
 *
 * Never panics. Silently evicts when full or expired.
 */

interface Entry<V> {
  readonly value: V;
  readonly expiresAt: number;
}

export interface CacheOptions {
  readonly maxEntries: number;
  readonly ttlMs: number;
}

export class TimedCache<K extends string, V> {
  private readonly store = new Map<K, Entry<V>>();
  private readonly maxEntries: number;
  private readonly ttlMs: number;

  public constructor(options: CacheOptions) {
    this.maxEntries = Math.max(1, Math.floor(options.maxEntries));
    this.ttlMs = Math.max(0, Math.floor(options.ttlMs));
  }

  public get(key: K): V | undefined {
    const hit = this.store.get(key);
    if (!hit) {
      return undefined;
    }
    if (Date.now() >= hit.expiresAt) {
      this.store.delete(key);
      return undefined;
    }
    // Refresh LRU position.
    this.store.delete(key);
    this.store.set(key, hit);
    return hit.value;
  }

  public set(key: K, value: V): void {
    if (this.store.has(key)) {
      this.store.delete(key);
    }
    while (this.store.size >= this.maxEntries) {
      const oldest = this.store.keys().next();
      if (oldest.done) {
        break;
      }
      this.store.delete(oldest.value);
    }
    this.store.set(key, {
      value,
      expiresAt: Date.now() + this.ttlMs,
    });
  }

  public has(key: K): boolean {
    return this.get(key) !== undefined;
  }

  public delete(key: K): void {
    this.store.delete(key);
  }

  public clear(): void {
    this.store.clear();
  }

  public size(): number {
    return this.store.size;
  }
}

/**
 * Fast non-crypto hash for short strings. Used to key the blast cache.
 * FNV-1a, 32-bit. Good enough to detect file content changes.
 */
export function quickHash(input: string): string {
  let hash = 0x811c9dc5;
  for (let i = 0; i < input.length; i++) {
    hash ^= input.charCodeAt(i);
    hash = Math.imul(hash, 0x01000193);
  }
  // Force unsigned, return padded hex.
  const unsigned = hash >>> 0;
  return unsigned.toString(16).padStart(8, "0");
}
