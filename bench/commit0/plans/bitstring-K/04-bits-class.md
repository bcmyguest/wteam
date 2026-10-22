# 04: Bits — complete the immutable Bits class

**What to build:** The construction and dtype-setter half of `Bits` in `bitstring/bits.py`, built on
the tokenizer (ticket 02), the converters (ticket 03) and the getters, internals, search and `pp`
that ticket 09 adds. Do not reimplement anything 02, 03 or 09 own. If one of their functions is
wrong, fix the call site here and report the bug in the handoff.

- construction: `_setauto` / `_setauto_no_length_or_offset` for every documented auto type:
  comma-separated format strings via `utils.tokenparser` + `bitstore_helpers.bitstore_from_token`,
  ints → zeroed bits, bool, iterables, `array.array`, files/`filename=`/`io` objects via mmap
  (`_setfile`) with `length`/`offset`, bitarray, other bitstrings. Keyword initialisers (`uint=,
  int=, float=, ue=, bytes=, bin=, ...`) with `length`/`offset` validation → CreationError.
  `fromstring`.
- every dtype **setter** registered in `__init__.py:212-281`: (u)int(le/be/ne), float(le),
  bfloat(le), bits, bool, bytes, oct, pad, ue/se/uie/sie, p3binary/p4binary, e4m3/e5m2/e3m2/e2m3/
  e2m1/e8m0 mxfp, mxint. Each forwards to the matching `bitstore_helpers` converter.
- token reading: `_readtoken`, `unpack`.

Behaviour reference: `doc/bits.rst`, `doc/interpretation.rst`, `doc/exotic_floats.rst`,
`doc/introduction.rst`.

**Blocked by:** 02, 03, 09

**Status:** ready-for-agent

**Files:** bitstring/bits.py, tests/test_bits_construction.py

**Context:**
`bitstring/bits.py:412  Bits._initialise(auto, length, offset, **kwargs)  — keyword dispatch via dtype_register`
`bitstring/bits.py:460  Bits._setauto_no_length_or_offset(s)  — auto initialiser, literals only so far`
`bitstring/bits.py:484  Bits._setauto(s, length, offset)  — auto with length/offset`
`bitstring/bits.py:496  Bits._setfile(filename, length, offset)  — mmap file source (stub)`
`bitstring/bits.py:536-660  setter stubs (_setint, _setuintbe, _setintbe, _setue/_setse/_setuie/_setsie, _setoct)`
`bitstring/bits.py:707  Bits._readtoken(name, pos, length) -> (value, newpos)  — stub`
`bitstring/bits.py:767  Bits.unpack(fmt, **kwargs)  — stub`
`bitstring/bits.py:1006  Bits.fromstring(s)  — stub`
`bitstring/bits.py:1012-1102  setter stubs (_setintle ... _setmxint)`
`bitstring/utils.py:24  tokenparser(fmt, keys=()) -> (stretchy, tokens)  — ticket 02`
`bitstring/bitstore_helpers.py  *2bitstore(value, length...) -> BitStore, bitstore_from_token(name, length, value)  — ticket 03`
`bitstring/dtypes.py:9  Dtype(token).get_fn/read_fn/set_fn  — dtype dispatch`
`bitstring/__init__.py:212-281  dtype_definitions  — names of every _set*/_get* method`
`bitstring/__main__.py:35  b1._readtoken(fp, 0, len)  — CLI caller`
`bitstring/bitstream.py:247  ConstBitStream.read  — caller of _readtoken (ticket 06)`

**Verify:** python -m pytest tests/test_bits_construction.py -q

- [ ] `Bits('uint:8=3, int:4=-1, 0b1').bin == '0000001111111'`; `Bits(5).bin == '00000'`; `Bits(bytes=b'\x0f', offset=4).bin == '1111'`.
- [ ] `Bits(float=0.5, length=32).float == 0.5`; `Bits(ue=3).ue == 3`; `Bits(intle=-2, length=16).intle == -2`; `Bits(uint=3)` raises CreationError.
- [ ] `Bits('0x000f').unpack('uint:12, bin') == [0, '1111']`.
- [ ] `Bits(filename=path, offset=4, length=8)` reads those bits from a temp file; `Bits([1, 0, True])` and `Bits(array.array('B', [1]))` build as documented.
- [ ] `Bits.fromstring('int:8=-1').int == -1`.
