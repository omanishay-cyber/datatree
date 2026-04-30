# Encryption Reference

Complete encryption patterns for Electron + TypeScript desktop applications.

---

## Envelope Encryption Flow

```
                    ┌──────────────────────────┐
                    │      User's Data          │
                    │  (product records, etc.)   │
                    └────────────┬─────────────┘
                                 │
                                 v
                    ┌──────────────────────────┐
                    │  Encrypt with DEK         │
                    │  Algorithm: AES-256-GCM   │
                    │  Input: plaintext + DEK   │
                    │  Output: ciphertext +     │
                    │          IV + authTag      │
                    └────────────┬─────────────┘
                                 │
              ┌──────────────────┼──────────────────┐
              v                  v                   v
        [Ciphertext]          [IV]             [Auth Tag]
        (stored in DB)     (stored with      (stored with
                           ciphertext)       ciphertext)
                                 │
                                 v
                    ┌──────────────────────────┐
                    │  DEK (Data Encryption Key)│
                    │  Random 256-bit key       │
                    │  Unique per database or   │
                    │  per encryption session   │
                    └────────────┬─────────────┘
                                 │
                                 v
                    ┌──────────────────────────┐
                    │  Encrypt DEK with KEK     │
                    │  Algorithm: AES-256-GCM   │
                    │  Output: encrypted DEK    │
                    └────────────┬─────────────┘
                                 │
                                 v
                    ┌──────────────────────────┐
                    │  KEK (Key Encryption Key) │
                    │                           │
                    │  Option A: Derived from   │
                    │  master password via       │
                    │  Argon2id                  │
                    │                           │
                    │  Option B: Protected by   │
                    │  Electron safeStorage      │
                    │  (OS keychain)             │
                    └──────────────────────────┘
```

### Why Envelope Encryption?
- **Key rotation**: Change the KEK without re-encrypting all data (just re-encrypt the DEK).
- **Multiple access**: Different users/devices can each wrap the same DEK with their own KEK.
- **Separation of concerns**: Data encryption is independent of key management.

---

## DEK/KEK Lifecycle

### DEK (Data Encryption Key)
1. **Generation**: `crypto.randomBytes(32)` — 256 bits of cryptographically secure randomness.
2. **Usage**: Encrypts/decrypts all data records in a given scope (e.g., one database).
3. **Storage**: NEVER stored in plaintext. Always encrypted with the KEK.
4. **Rotation**: Generate new DEK, re-encrypt all data, encrypt new DEK with KEK, delete old DEK.
5. **Destruction**: Overwrite buffer with zeros when done: `dek.fill(0)`.

### KEK (Key Encryption Key)
1. **Derivation** (password-based): Argon2id with salt, memory, time, and parallelism parameters.
2. **Storage** (keychain-based): `safeStorage.encryptString(kek.toString('hex'))`.
3. **Rotation**: Decrypt DEK with old KEK, re-encrypt DEK with new KEK.
4. **Recovery**: Recovery codes must be generated at setup time (see Recovery Codes section).

---

## Electron safeStorage API

```typescript
import { safeStorage } from 'electron';
import crypto from 'crypto';
import fs from 'fs';

// Check availability (must be after app.ready)
if (!safeStorage.isEncryptionAvailable()) {
  throw new Error('OS keychain encryption is not available');
}

// Encrypt a key for storage
function storeKey(keyName: string, keyValue: Buffer): void {
  const encrypted = safeStorage.encryptString(keyValue.toString('hex'));
  const keyPath = getKeyPath(keyName);
  fs.writeFileSync(keyPath, encrypted, { mode: 0o600 });
}

// Decrypt a stored key
function loadKey(keyName: string): Buffer {
  const keyPath = getKeyPath(keyName);
  const encrypted = fs.readFileSync(keyPath);
  const hexString = safeStorage.decryptString(encrypted);
  return Buffer.from(hexString, 'hex');
}

// Key file location (in app's userData directory)
function getKeyPath(keyName: string): string {
  const { app } = require('electron');
  return path.join(app.getPath('userData'), 'keys', `${keyName}.key`);
}
```

### Platform Backends
- **Windows**: DPAPI (Data Protection API) — tied to the Windows user account
- **macOS**: Keychain Services — requires user login, optionally Touch ID
- **Linux**: libsecret (GNOME Keyring, KWallet) — requires desktop session

---

## AES-256-GCM Implementation

```typescript
import crypto from 'crypto';

const ALGORITHM = 'aes-256-gcm';
const IV_LENGTH = 12;        // 96 bits — recommended for GCM
const AUTH_TAG_LENGTH = 16;  // 128 bits — maximum security

interface EncryptedPayload {
  ciphertext: Buffer;
  iv: Buffer;
  authTag: Buffer;
}

function encrypt(plaintext: Buffer, key: Buffer): EncryptedPayload {
  const iv = crypto.randomBytes(IV_LENGTH);
  const cipher = crypto.createCipheriv(ALGORITHM, key, iv, {
    authTagLength: AUTH_TAG_LENGTH,
  });

  const ciphertext = Buffer.concat([
    cipher.update(plaintext),
    cipher.final(),
  ]);

  return {
    ciphertext,
    iv,
    authTag: cipher.getAuthTag(),
  };
}

function decrypt(payload: EncryptedPayload, key: Buffer): Buffer {
  const decipher = crypto.createDecipheriv(ALGORITHM, payload.iv.length === IV_LENGTH ? ALGORITHM : ALGORITHM, key, payload.iv, {
    authTagLength: AUTH_TAG_LENGTH,
  });

  // IMPORTANT: Set auth tag BEFORE decrypting
  decipher.setAuthTag(payload.authTag);

  const plaintext = Buffer.concat([
    decipher.update(payload.ciphertext),
    decipher.final(), // Throws if auth tag verification fails
  ]);

  return plaintext;
}

// Corrected decrypt function
function decryptData(payload: EncryptedPayload, key: Buffer): Buffer {
  const decipher = crypto.createDecipheriv(ALGORITHM, key, payload.iv, {
    authTagLength: AUTH_TAG_LENGTH,
  });
  decipher.setAuthTag(payload.authTag);
  const plaintext = Buffer.concat([
    decipher.update(payload.ciphertext),
    decipher.final(),
  ]);
  return plaintext;
}

// Serialize for storage (e.g., in SQLite BLOB or file)
function serializePayload(payload: EncryptedPayload): Buffer {
  // Format: [IV (12 bytes)][AuthTag (16 bytes)][Ciphertext (variable)]
  return Buffer.concat([payload.iv, payload.authTag, payload.ciphertext]);
}

function deserializePayload(data: Buffer): EncryptedPayload {
  return {
    iv: data.subarray(0, IV_LENGTH),
    authTag: data.subarray(IV_LENGTH, IV_LENGTH + AUTH_TAG_LENGTH),
    ciphertext: data.subarray(IV_LENGTH + AUTH_TAG_LENGTH),
  };
}
```

---

## Key Derivation — Argon2id

Argon2id is the recommended key derivation function. It resists both GPU
and side-channel attacks.

```typescript
import argon2 from 'argon2';
import crypto from 'crypto';

// Derive a KEK from a master password
async function deriveKEK(password: string, salt?: Buffer): Promise<{ kek: Buffer; salt: Buffer }> {
  const derivedSalt = salt ?? crypto.randomBytes(16);

  const kek = await argon2.hash(password, {
    type: argon2.argon2id,
    memoryCost: 65536,    // 64 MB — resist GPU attacks
    timeCost: 3,          // 3 iterations
    parallelism: 4,       // 4 threads
    hashLength: 32,       // 256-bit output for AES-256
    salt: derivedSalt,
    raw: true,            // Return raw bytes, not encoded string
  });

  return { kek: Buffer.from(kek), salt: derivedSalt };
}

// PBKDF2 fallback (if argon2 native addon is not available)
function deriveKEK_PBKDF2(password: string, salt?: Buffer): { kek: Buffer; salt: Buffer } {
  const derivedSalt = salt ?? crypto.randomBytes(16);

  const kek = crypto.pbkdf2Sync(
    password,
    derivedSalt,
    600_000,    // OWASP minimum for PBKDF2-SHA256 (2023+)
    32,         // 256-bit output
    'sha256'
  );

  return { kek, salt: derivedSalt };
}
```

### Parameter Guidance
| Parameter | Minimum | Recommended | Notes |
|-----------|---------|-------------|-------|
| `memoryCost` | 19456 (19 MB) | 65536 (64 MB) | Higher = more GPU-resistant |
| `timeCost` | 2 | 3 | Higher = slower but more secure |
| `parallelism` | 1 | 4 | Match to available CPU cores |
| `hashLength` | 32 | 32 | 256 bits for AES-256 |

---

## Key Rotation Procedure

```typescript
async function rotateKeys(
  db: Database,
  oldKEK: Buffer,
  newKEK: Buffer
): Promise<void> {
  // 1. Decrypt current DEK with old KEK
  const encryptedDEK = db.getEncryptedDEK();
  const dek = decryptData(deserializePayload(encryptedDEK), oldKEK);

  // 2. Encrypt DEK with new KEK
  const newEncryptedDEK = encrypt(dek, newKEK);
  db.storeEncryptedDEK(serializePayload(newEncryptedDEK));

  // 3. Zero out old key material
  oldKEK.fill(0);
  dek.fill(0); // DEK is re-encrypted, original buffer not needed

  // NOTE: Data does NOT need to be re-encrypted
  // Only the DEK wrapping changes
}

async function fullDataRotation(
  db: Database,
  kek: Buffer
): Promise<void> {
  // For full DEK rotation, ALL data must be re-encrypted
  const oldDEK = decryptDEKFromStorage(db, kek);
  const newDEK = crypto.randomBytes(32);

  // Re-encrypt all records
  const records = db.getAllEncryptedRecords();
  for (const record of records) {
    const plaintext = decryptData(deserializePayload(record.data), oldDEK);
    const reEncrypted = encrypt(plaintext, newDEK);
    db.updateRecord(record.id, serializePayload(reEncrypted));
  }

  // Encrypt and store new DEK
  const wrappedDEK = encrypt(newDEK, kek);
  db.storeEncryptedDEK(serializePayload(wrappedDEK));

  // Zero out old DEK
  oldDEK.fill(0);
  newDEK.fill(0);
}
```

---

## Recovery Codes

Generate recovery codes at setup time so the user can regain access if they
forget their master password.

```typescript
function generateRecoveryCodes(count: number = 8): string[] {
  const codes: string[] = [];
  for (let i = 0; i < count; i++) {
    // Generate 10-character alphanumeric codes, grouped for readability
    const raw = crypto.randomBytes(5).toString('hex').toUpperCase();
    codes.push(`${raw.slice(0, 5)}-${raw.slice(5)}`);
  }
  return codes;
}

// Store hashed recovery codes (never store plaintext)
async function storeRecoveryCodes(codes: string[], db: Database): Promise<void> {
  for (const code of codes) {
    const hash = crypto.createHash('sha256').update(code).digest('hex');
    db.storeRecoveryCodeHash(hash);
  }
}

// Verify a recovery code
async function verifyRecoveryCode(code: string, db: Database): Promise<boolean> {
  const hash = crypto.createHash('sha256').update(code.toUpperCase().trim()).digest('hex');
  const found = db.findRecoveryCodeHash(hash);
  if (found) {
    db.markRecoveryCodeUsed(hash); // One-time use
    return true;
  }
  return false;
}
```

### Recovery Code Rules
- Generate 8 codes at account/encryption setup
- Display once, instruct user to save securely
- Hash with SHA-256 before storing (recovery codes are high-entropy, no need for slow hash)
- Each code is single-use — mark as consumed after verification
- If all codes are consumed, require master password to generate new ones
