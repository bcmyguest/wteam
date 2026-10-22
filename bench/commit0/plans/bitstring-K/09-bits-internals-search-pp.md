# 09: Bits — internals, getters, search/cut/split and pp (no converters needed)

**What to build:** The half of `Bits` in `bitstring/bits.py` that runs on the tracer alone, without
`utils.tokenparser` (ticket 02) or `bitstore_helpers` converters (ticket 03). Build every test
bitstring from literal strings (`'0b..'`, `'0x..'`, `'0o..'`), `bytes`, ints or other Bits: these
already work through `_setauto_no_length_or_offset` (bits.py:460). Do not touch `utils.py` or
`bitstore_helpers.py`, and leave the dtype **setters** (`_setint`, `_setfloatbe`, `_setue`, ...) and
the format-string paths (`_setauto` for comma-separated strings, `_readtoken`, `unpack`,
`fromstring`) to ticket 04.

- getters (pure reads of `self._bitstore`): `_getint`, `_getuintbe/_getintbe/_getuintle/_getintle`
  (InterpretError unless whole bytes), `_getfloatbe/_getfloatle` (16/32/64 bits, struct),
  `_getbfloatbe/_getbfloatle`, `_getbool`, `_getbits`, `_getpad`, `_getoct`, `_getbytes_printable`,
  `_readue/_readse/_readuie/_readsie` returning `(value, newpos)` with ReadError on running out,
  and `_getue/_getse/_getuie/_getsie` (InterpretError if the code doesn't fill the bitstring).
  The p3/p4binary, mxfp and mxint getters may use `fp8.py`/`mxfp.py` format instances directly.
- internals: `_addright`, `_addleft`, `_truncateleft/right`, `_insert`, `_overwrite`, `_delete`,
  `_reversebytes`, `_invert`, `_invert_all`, `_ilshift`, `_irshift`, `_imul`, `_validate_slice`
  (negative indices, IndexError/ValueError), all correct under `options.lsb0`.
- public: `find`/`_find_msb0`, `findall`, `rfind`/`_rfind_msb0` (honour `bytealigned` arg and
  `options.bytealigned`; settle LSB0 positions, see queue key `lsb0-find`), `cut`, `split`, `join`,
  `tobytes`, `tobitarray`, `tofile`, `startswith`, `endswith`, `all`, `any`, `count`, `copy`,
  `pp`/`_pp`/`_chars_per_group`/`_bits_per_char` (format in `doc/bits.rst` pp section, colours via
  `Colour(not options.no_color)`).

Behaviour reference: `doc/bits.rst`, `doc/interpretation.rst`, `doc/exp-golomb.rst`.

**Blocked by:** 01

**Status:** ready-for-agent

**Files:** bitstring/bits.py, tests/test_bits_internals.py

**Context:**
`bitstring/bits.py:25  class Bits  — __slots__ ('_bitstore', '_filename')`
`bitstring/bits.py:460  Bits._setauto_no_length_or_offset(s)  — literal/bytes/int/bool/Bits construction (works now)`
`bitstring/bits.py:514-675  getter stubs (_getbytes_printable, _getint, ..., _readue..._readsie, _getoct)`
`bitstring/bits.py:711-765  internal stubs (_addright ... _validate_slice)`
`bitstring/bits.py:784-1001  public stubs (find ... copy, pp)`
`bitstring/bits.py:1027-1117  remaining getters (_getbfloat*, _getbool, _getbits, _getpad, exotic floats, _getue/_getse...)`
`bitstring/bitstore.py:6  BitStore  — getindex/getslice_msb0/setall/find/rfind/findall_msb0 (ticket 01)`
`bitstring/bitstring_options.py:26  Colour(use_colour)  — pp colours`
`bitstring/bitstring_options.py  Options.lsb0 / set_lsb0  — rebinds lsb0 variants`
`bitstring/dtypes.py:9  Dtype(token)  — used by pp for fmt`
`bitstring/fp8.py:32, bitstring/mxfp.py:49-55  format instances  — exotic float decode`

**Verify:** python -m pytest tests/test_bits_internals.py -q

- [ ] `Bits('0xc3e').find('0b1111') == (6,)`; `list(Bits('0b0101').findall('0b1')) == [1, 3]`; `Bits('0b0101').rfind('0b1') == (3,)`; `find` with `bytealigned=True` skips non-aligned hits.
- [ ] `list(Bits('0x0f0f').cut(4))` gives four 4-bit pieces; `list(Bits('0b001001').split('0b1'))` matches doc example; `Bits('0b1').join(['0b0', '0b0']).bin == '010'`.
- [ ] `Bits('0b0011').count(1) == 2`; `all`/`any` with `pos`; `startswith`/`endswith`; `Bits('0b1').tobytes() == b'\x80'`.
- [ ] `Bits('0xfffe').int == -2`; `Bits('0xfeff').intle == -2`; `Bits('0x3f000000').float == 0.5`; `Bits('0b00100').ue == 3`; `Bits('0b1').hex` raises InterpretError.
- [ ] `pp()` output for `Bits('0x12345')` matches the format shown in `doc/bits.rst`.
- [ ] With `options.lsb0 = True`, slicing and find report LSB0 positions as documented.
