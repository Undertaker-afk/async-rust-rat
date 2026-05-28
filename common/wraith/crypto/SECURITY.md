# wraith-crypto Security Documentation

This document describes the cryptographic security properties, design decisions,
and security guidelines for the wraith-crypto crate.

## Cryptographic Primitives

### Summary

| Primitive | Algorithm | Key Size | Security Level |
|-----------|-----------|----------|----------------|
| Key Exchange | X25519 | 256-bit | ~128-bit |
| AEAD | XChaCha20-Poly1305 | 256-bit | 256-bit |
| Hash | BLAKE3 | N/A | 256-bit |
| KDF | BLAKE3-based HKDF | 256-bit | 256-bit |
| Handshake | Noise_XX | 256-bit | ~128-bit (DH) |
| Key Hiding | Elligator2 | 256-bit | Indistinguishable |

### X25519 Key Exchange

**Implementation:** `x25519-dalek` crate with `curve25519-dalek` backend.

**Security Properties:**
- 128-bit security level (256-bit curve)
- Constant-time scalar multiplication
- Automatic clamping per RFC 7748
- Low-order point rejection

**Key Management:**
- Private keys are zeroized on drop via `zeroize` crate
- Keys are clamped before use to prevent small-subgroup attacks

**Test Vectors:** RFC 7748 test vectors implemented in `tests/vectors.rs`.

### XChaCha20-Poly1305 AEAD

**Implementation:** `chacha20poly1305` crate.

**Security Properties:**
- 256-bit key security
- 192-bit nonce (random nonces safe, no counter management)
- 128-bit authentication tag
- AEAD provides confidentiality and integrity

**Nonce Handling:**
- Extended 192-bit nonce allows random nonce selection
- Never reuse (key, nonce) pairs - this breaks security completely
- In Double Ratchet: deterministic nonces from message numbers (safe due to unique keys)

**Tag Verification:**
- Constant-time tag comparison
- Decryption fails atomically if tag verification fails

### BLAKE3 Hashing

**Implementation:** `blake3` crate.

**Security Properties:**
- 256-bit output
- Collision resistance: 128-bit
- Preimage resistance: 256-bit
- Tree-parallelizable for high performance

**KDF Construction:**
- HKDF-like extract-then-expand using BLAKE3
- Context strings for domain separation
- Supports arbitrary output lengths

### Noise_XX Handshake

**Implementation:** `snow` crate with custom wrapper.

**Pattern:** Noise_XX(s, rs)
```
-> e
<- e, ee, s, es
-> s, se
```

**Security Properties:**
- Mutual authentication of static keys
- Forward secrecy from ephemeral keys
- Identity hiding: responder identity hidden from passive attackers
- Resistance to key compromise impersonation (KCI)

**Session Keys:**
- Handshake produces 64 bytes of key material
- Split into send/recv keys for each party
- Chain key derived for Double Ratchet initialization

### Elligator2 Encoding

**Implementation:** `curve25519-elligator2` crate.

**Security Properties:**
- Public keys indistinguishable from random bytes
- Prevents protocol fingerprinting via public key patterns
- ~50% of keys are encodable (retry until success)

**Usage:**
- Generate keypairs via `generate_encodable_keypair()`
- Never use `encode_public_key()` on existing keys (use generation)
- Representatives decode deterministically

### Double Ratchet

**Implementation:** Custom implementation following Signal specification.

**Security Properties:**
- Forward secrecy: compromise of current keys doesn't reveal past messages
- Post-compromise security: security restored after DH ratchet
- Message ordering: out-of-order delivery supported
- Message skipping: limited by MAX_SKIP to prevent DoS

**Ratcheting:**
- Symmetric ratchet: KDF-based chain advancement
- DH ratchet: triggered on direction change
- Skipped keys: stored temporarily for out-of-order messages

## Constant-Time Operations

### Requirements

All operations on secret data MUST be constant-time:
- Key comparisons
- Tag verification
- Conditional operations on secrets

### Implementation

The `constant_time` module provides:
- `ct_eq()` - constant-time byte slice comparison
- `ct_select()` - constant-time conditional selection
- `verify_16/32/64()` - fixed-size comparisons

**Verification:**
- Comparison timing tests in `tests/vectors.rs`
- Uses `subtle` crate for underlying operations

## Memory Security

### Zeroization

All sensitive data types implement `ZeroizeOnDrop`:
- `PrivateKey`
- `SharedSecret`
- `AeadKey`
- `MessageKey`
- `SymmetricRatchet`
- `DoubleRatchet`

**Behavior:**
- Keys are overwritten with zeros when dropped
- Prevents key recovery from freed memory
- Works even with panics (Drop still called)

### Memory Allocation

- No heap allocation in hot crypto paths where possible
- Stack-allocated keys and intermediate values
- Explicit zeroization before reuse

## Security Considerations

### Do's

1. **Generate keys properly:**
   ```rust
   let private = PrivateKey::generate(&mut OsRng);
   ```

2. **Use Elligator2 for key hiding:**
   ```rust
   let (private, repr) = generate_encodable_keypair(&mut OsRng);
   ```

3. **Complete full handshake:**
   ```rust
   // All 3 messages must complete
   let msg1 = initiator.write_message(&[])?;
   let msg2 = responder.write_message(&[])?;
   let msg3 = initiator.write_message(&[])?;
   ```

4. **Initialize Double Ratchet correctly:**
   ```rust
   // Initiator needs peer's DH public
   let alice = DoubleRatchet::new_initiator(&mut rng, &shared, peer_dh);
   // Responder provides their own DH
   let bob = DoubleRatchet::new_responder(&shared, my_dh);
   ```

### Don'ts

1. **Never reuse nonces with same key:**
   ```rust
   // BAD: Same nonce reuse
   key.encrypt(&nonce, plaintext1, aad)?;
   key.encrypt(&nonce, plaintext2, aad)?; // CATASTROPHIC
   ```

2. **Never skip handshake steps:**
   ```rust
   // BAD: Incomplete handshake
   let msg1 = initiator.write_message(&[])?;
   let keys = initiator.into_session_keys()?; // WILL FAIL
   ```

3. **Never use predictable randomness:**
   ```rust
   // BAD: Weak RNG
   let mut rng = rand::thread_rng(); // OK for testing only

   // GOOD: Cryptographic RNG
   let mut rng = rand_core::OsRng;
   ```

4. **Never log or print keys:**
   ```rust
   // BAD: Exposes key material
   println!("Key: {:?}", key.as_bytes());
   ```

## Threat Model

### Protected Against

- **Passive eavesdropping:** All traffic encrypted with AEAD
- **Active man-in-the-middle:** Mutual authentication via Noise_XX
- **Key compromise (past):** Forward secrecy via ephemeral keys
- **Key compromise (current):** Post-compromise security via DH ratchet
- **Protocol fingerprinting:** Elligator2 makes keys look random
- **Timing attacks:** Constant-time operations on secrets
- **Memory forensics:** Zeroization on drop

### Not Protected Against

- **Compromised endpoints:** If malware on sender/receiver, all bets off
- **Side channels:** Cache timing, power analysis (implementation-dependent)
- **Metadata:** Message timing, sizes, endpoints visible
- **Quantum adversaries:** X25519 is not post-quantum secure

## Auditing

### Test Coverage

- **Unit tests:** 79 tests in library
- **Integration tests:** 24 tests in vectors.rs
- **RFC vectors:** X25519 RFC 7748 vectors
- **Interop:** BLAKE3 official test vectors

### Running Security Tests

```bash
# All crypto tests
cargo test -p wraith-crypto

# Test vectors specifically
cargo test -p wraith-crypto --test vectors

# With output
cargo test -p wraith-crypto -- --nocapture
```

### Performance Benchmarks

```bash
# Run all benchmarks
cargo bench -p wraith-crypto

# Specific benchmark group
cargo bench -p wraith-crypto -- aead
```

## Dependencies

All cryptographic dependencies are well-audited crates:

| Crate | Audit Status | Notes |
|-------|--------------|-------|
| `chacha20poly1305` | RustCrypto, widely reviewed | AEAD |
| `x25519-dalek` | Dalek, widely reviewed | Key exchange |
| `blake3` | Official impl, audited | Hashing |
| `snow` | Noise impl, reviewed | Handshake |
| `curve25519-elligator2` | Newer, review ongoing | Elligator2 |
| `subtle` | RustCrypto, widely reviewed | Constant-time |
| `zeroize` | RustCrypto, widely reviewed | Memory security |

## Version History

| Version | Changes |
|---------|---------|
| 0.1.5 | Initial Phase 2 implementation |

## References

- [Noise Protocol Framework](https://noiseprotocol.org/noise.html)
- [RFC 7748: X25519](https://datatracker.ietf.org/doc/html/rfc7748)
- [Signal Double Ratchet](https://signal.org/docs/specifications/doubleratchet/)
- [Elligator2 Paper](https://elligator.cr.yp.to/)
- [BLAKE3 Specification](https://github.com/BLAKE3-team/BLAKE3-specs)
