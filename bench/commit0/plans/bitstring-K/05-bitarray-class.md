# 05: BitArray — mutable subclass methods

**What to build:** Complete `BitArray` in `bitstring/bitarray_.py`: the stubbed public methods
and the deleted helpers the surviving dunder methods call.

- re-add `_setitem_int(key, value)`, `_setitem_slice(key, value)` (used by `__setitem__`
  at bitarray_.py:146), `_append`, `_prepend` (used by `__iadd__` and bitstream.py:503),
  and anything else `__setattr__` / `__delitem__` / `__ilshift__` etc. reference.
- `copy`, `replace` (returns count; honours `bytealigned`, `count`, start/end), `insert`,
  `overwrite`, `append`, `prepend`, `reverse`, `set` (single pos or iterable, negative
  indices), `invert`, `ror`, `rol`, `byteswap` (int / iterable / struct-format `fmt`,
  `repeat`), `clear`.
- `__setattr__` keyword-property assignment (`a.uint8 = 3`, `a.hex = 'ff'`) must work for
  every registered dtype.
- Correct under `options.lsb0` (e.g. `insert`/`overwrite` positions count from the right).

Behaviour reference: `doc/bitarray.rst`, `spec.pdf`.

**Blocked by:** 04

**Status:** ready-for-agent

**Files:** bitstring/bitarray_.py

**Context:**
`bitstring/bitarray_.py:12  class BitArray(Bits)`
`bitstring/bitarray_.py:116  __setattr__(attribute, value)  — dtype property setting`
`bitstring/bitarray_.py:130  __iadd__  — calls self._append`
`bitstring/bitarray_.py:146  __setitem__  — calls _setitem_int / _setitem_slice`
`bitstring/bitarray_.py:220-358  stubs: replace, insert, overwrite, append, prepend, reverse, set, invert, ror, rol, byteswap, clear`
`bitstring/bits.py:610-664  Bits._addright/_addleft/_insert/_overwrite/_delete/_reversebytes/_invert/_validate_slice  — base internals (ticket 04)`
`bitstring/bits.py:683  Bits.find / findall  — used by replace`
`bitstring/bitstore.py:6  BitStore  — mutable storage (immutable=False)`
`bitstring/utils.py:11-18  STRUCT_PACK_RE, BYTESWAP_STRUCT_PACK_RE, PACK_CODE_SIZE  — byteswap fmt parsing`
`bitstring/bitstream.py:385  class BitStream(ConstBitStream, BitArray)  — subclass (ticket 06)`

- [ ] `a = BitArray('0b000'); a.set(1, -1); a.bin == '001'`; `a.invert(0)`; `a.reverse()` match doc examples.
- [ ] `a = BitArray('0b00010001'); a.replace('0b1', '0b11') == 2` and the result is `'0b0001100011'`.
- [ ] `insert`, `overwrite`, `append`, `prepend`, `ror`, `rol` match `doc/bitarray.rst`.
- [ ] `BitArray('0x12345678').byteswap() == 1` gives `0x78563412`; `byteswap('h')` swaps 16-bit chunks.
- [ ] `a = BitArray(8); a.uint = 3; a.hex == '03'`; `a[0:4] = '0xf'`; `del a[0]` work; `a.clear()` empties.
