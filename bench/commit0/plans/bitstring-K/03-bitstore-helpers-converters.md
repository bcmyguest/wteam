# 03: bitstore_helpers — value → BitStore converters and bitstore_from_token

**What to build:** Every function in `bitstring/bitstore_helpers.py` that turns a Python value
into a `BitStore`, with the signatures ticket 01 declared (see the comment block it wrote at
the top of the module). Implement:

- integers: `uint2bitstore`, `int2bitstore`, `uintle2bitstore`, `intle2bitstore`
  (big-endian variants as needed), with CreationError/ValueError on out-of-range values and
  non-whole-byte lengths for le/be variants.
- floats: `float2bitstore(f, length, big_endian)` for 16/32/64 bits (overflow → ±inf for
  16/32 as upstream), `bfloat2bitstore`, little-endian forms.
- exp-Golomb: `ue2bitstore`, `se2bitstore`, `uie2bitstore`, `sie2bitstore`
  (`doc/exp-golomb.rst`; negative ue/uie raise CreationError).
- exotic floats: p3binary/p4binary, e4m3/e5m2 (honouring `options.mxfp_overflow`),
  e3m2/e2m3/e2m1 MXFP, `e8m0mxfp2bitstore` (uses `e8m0mxfp_allowed_values`), `mxint2bitstore`,
  using the formats/LUTs from `fp8.py`/`mxfp.py`.
- `bin2bitstore`, `hex2bitstore`, `oct2bitstore`, `tidy_input_string` already exist from 01:
  keep them working (whitespace/underscores/`0x` prefix tolerated, bad chars raise).
- `bitstore_from_token(name, token_length, value) -> BitStore`: builds from a parsed token
  via `Dtype(name, token_length).set_fn` / the registry, handling `bits`, `bytes`, `bool`
  (`'True'`/`'1'` strings), `pad`, literals (`literal_bit_funcs`), and length mismatches
  (CreationError). Cache with `functools.lru_cache(CACHE_SIZE)` as upstream does.

**Blocked by:** 01

**Status:** ready-for-agent

**Files:** bitstring/bitstore_helpers.py

**Context:**
`bitstring/bitstore_helpers.py:11  CACHE_SIZE = 256`
`bitstring/bitstore_helpers.py:13  tidy_input_string(s: str) -> str`
`bitstring/bitstore_helpers.py:16  e8m0mxfp_allowed_values  — list of 2**-127..2**127`
`bitstring/bitstore_helpers.py:17  literal_bit_funcs: Dict[str, Callable[..., BitStore]]`
`bitstring/bitstore.py:6  BitStore  — storage wrapper (ticket 01)`
`bitstring/fp8.py:32-33  p4binary_fmt, p3binary_fmt  — Binary8Format instances`
`bitstring/mxfp.py:49-55  e2m1/e2m3/e3m2/e4m3*/e5m2* MXFPFormat instances`
`bitstring/bitstring_options.py:5  Options().mxfp_overflow  — 'saturate' | 'overflow'`
`bitstring/dtypes.py:9  Dtype(name, length).set_fn / dtype_register.names  — token lookup`
`bitstring/methods.py:8  from bitstring.bitstore_helpers import bitstore_from_token  — caller`
`bitstring/bits.py:445-591  Bits._set* methods  — callers (ticket 04)`

- [ ] `int2bitstore(-1, 8)` equals `bin2bitstore('11111111')`; `uint2bitstore(256, 8)` raises CreationError.
- [ ] `ue2bitstore(3)` is `'00100'`, `se2bitstore(-2)` is `'00101'`, `uie2bitstore(3)` matches `doc/exp-golomb.rst`.
- [ ] `float2bitstore(1.0, 32, True)` is `0x3f800000`; little-endian form is `0x0000803f`.
- [ ] `bitstore_from_token('uint', 8, '3')` gives `0x03`; `bitstore_from_token('bool', None, 'True')` gives `'1'`; `bitstore_from_token('pad', 4, None)` gives four zero bits.
- [ ] `e4m3mxfp` encoding of a value above max saturates under `'saturate'` and gives NaN code under `'overflow'`.
