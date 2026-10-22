# 06: ConstBitStream / BitStream streaming methods and pack()

**What to build:** The streaming classes in `bitstring/bitstream.py` and the module-level
`pack` in `bitstring/methods.py`.

- `ConstBitStream`: `pos`/`bitpos`/`bytepos` via `_setbitpos`/`_getbitpos`/`_setbytepos`/
  `_getbytepos` (ValueError out of range, ByteAlignError for non-aligned bytepos), `read`
  (int bits, dtype token, Dtype, `'bits'`/`'bytes'` stretchy to end), `readlist`
  (format strings, lists, kwargs, at most one stretchy token), `readto`, `peek`,
  `peeklist`, `bytealign`, `find`/`rfind` setting `pos`, `append` (ConstBitStream raises?
  follow docs), `overwrite`, ReadError when reading past the end.
- `BitStream`: `prepend`, `insert` (default pos = current pos, pos moves past insert),
  `replace` (pos adjustments), keep `pos` consistent after `__iadd__`, `__setitem__`,
  `__delitem__`.
- `pack(fmt, *values, **kwargs) -> BitStream` using `utils.tokenparser` and
  `bitstore_helpers.bitstore_from_token`: positional values fill tokens without `=value`,
  keyword values replace names, `'bits'`/`'bytes'` stretchy tokens take a value of any
  length, wrong value count raises CreationError.

Behaviour reference: `doc/constbitstream.rst`, `doc/bitstream.rst`, `doc/functions.rst`.

`BitStream` inherits from `BitArray`, whose methods are being completed by the BitArray
ticket in parallel. Call them only through their existing signatures
(`bitarray_.py:220-358`, plus `_append`, `_prepend`, `_setitem_int`, `_setitem_slice`).
Full `BitStream` checks need the BitArray work merged; the integration ticket closes that seam.

**Blocked by:** 04

**Status:** ready-for-agent

**Files:** bitstring/bitstream.py, bitstring/methods.py

**Context:**
`bitstring/bitstream.py:10  class ConstBitStream(Bits)  — __slots__ includes _pos`
`bitstring/bitstream.py:58  ConstBitStream.__init__(auto, /, length, offset, pos=0, **kwargs)`
`bitstring/bitstream.py:105-119  _setbytepos/_getbytepos/_setbitpos/_getbitpos stubs`
`bitstring/bitstream.py:174-363  stubs: append, overwrite, find, rfind, read, readlist, readto, peek, peeklist, bytealign`
`bitstring/bitstream.py:385  class BitStream(ConstBitStream, BitArray)`
`bitstring/bitstream.py:507-568  stubs: prepend, insert, replace`
`bitstring/methods.py:10  pack(fmt: Union[str, List[str]], *values, **kwargs) -> BitStream`
`bitstring/bits.py:606  Bits._readtoken(name, pos, length) -> (value, newpos)  — reads one token (ticket 04)`
`bitstring/bits.py:666  Bits.unpack  — shares readlist parsing`
`bitstring/utils.py:24  tokenparser(fmt, keys) -> (stretchy, [(name, length, value)])  — (ticket 02)`
`bitstring/bitstore_helpers.py  bitstore_from_token(name, length, value) -> BitStore  — (ticket 03)`
`bitstring/dtypes.py:93  Dtype.read_fn  — (ticket 01)`

- [ ] `s = ConstBitStream('0x0102'); s.read(8).hex == '01'; s.pos == 8; s.read('uint:8') == 2`; reading again raises ReadError.
- [ ] `s.readlist('uint:4, bin:4, bits')` and `readlist('a, b', a='uint:4', ...)`-style kwargs match doc examples; `peek` leaves `pos` unchanged.
- [ ] `s = ConstBitStream('0x00ff00'); s.find('0xff'); s.pos == 8`; `readto('0xff')` returns up to and including the match; `bytealign()` returns bits skipped.
- [ ] `pack('uint:12, bits', 100, '0xffe')`, `pack(['bits', 'bin:3'], s, '111')` and `pack('uint:8=a, uint:8=b, uint:55=a', a=6, b=44)` produce the docstring results.
- [ ] `b = BitStream('0x00'); b.pos = 4; b.insert('0b11'); b.pos == 6`.
