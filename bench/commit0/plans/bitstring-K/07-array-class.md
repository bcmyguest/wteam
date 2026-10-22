# 07: Array — typed list-like container

**What to build:** Complete `Array` in `bitstring/array_.py`: the stubbed methods and the
deleted ones the surviving code calls.

- re-add: `_set_dtype`/`_get_dtype` and the `dtype` property (settable, raises on bad
  dtype), `itemsize`, `trailing_bits`, `data` handling, `extend`, `append`, `reverse`,
  `fromfile`, `tolist`, `_calculate_auto_scale` (for `scale='auto'` dtypes),
  `_apply_op_between_arrays`, `_eq_ne`, `_largest_values` logic if needed.
- stubs: `_create_element` (uses `dtype.build`/`set_fn`, raises on out-of-range value),
  `astype`, `insert`, `pop`, `byteswap`, `count` (NaN-aware for floats), `tobytes`, `tofile`,
  `pp`, `equals`, `_apply_op_to_all_elements(_inplace)`,
  `_apply_bitwise_op_to_all_elements(_inplace)`, `_promotetype`.
- arithmetic and comparison operators in array_.py:292-440 already route through these
  helpers; element-wise ops return a new Array (comparison → bool Array), integer
  overflow raises, division by an int Array promotes to float as documented.
- struct-style dtypes like `'>H'` and Dtypes with `scale` work.

Behaviour reference: `doc/array.rst`, `doc/dtypes.rst`, `doc/exotic_floats.rst` (scaled
Arrays), `spec.pdf`. Array uses `BitArray` only through its public API (`append`,
`overwrite`, `+=`, slicing, `del`), so it can be built while 04/05 land; full checks need
them merged.

**Blocked by:** 01

**Status:** ready-for-agent

**Files:** bitstring/array_.py

**Context:**
`bitstring/array_.py:17  ElementType = Union[float, str, int, bytes, bool, Bits]`
`bitstring/array_.py:18  options = Options()`
`bitstring/array_.py:65  Array.__init__(dtype, initializer=None, trailing_bits=None)  — calls _set_dtype, extend, fromfile, _calculate_auto_scale`
`bitstring/array_.py:88-290  stubs: _create_element ... _promotetype`
`bitstring/array_.py:292-440  operator dunders  — call _apply_op_* / _apply_op_between_arrays / _eq_ne`
`bitstring/array_.py:177  __repr__  — calls self.tolist()`
`bitstring/dtypes.py:9  Dtype(name, length, scale)  — .bitlength, .length, .read_fn, .build, .parse, .scale, .return_type, .is_signed`
`bitstring/bitarray_.py:12  BitArray  — data container (public API)`
`bitstring/bits.py:877  Bits._pp  — pp layout shared with Array.pp`
`bitstring/utils.py:13  SINGLE_STRUCT_PACK_RE  — '>H'-style dtype codes`

- [ ] `Array('>H', [1, 15, 105]).tolist() == [1, 15, 105]`; `Array('int5', [-9, 0, 4]).itemsize == 5`; `Array('uint8', [256])` raises.
- [ ] `a = Array('uint8', [1, 2]); a.append(3); a.extend([4]); a.pop() == 4; a.insert(0, 9); a.count(9) == 1`; `a.reverse()`.
- [ ] `(Array('int8', [1, 2]) + 1).tolist() == [2, 3]`; `(a < 2)` returns a bool Array; `Array('int8',[1,2]) + Array('int8',[3,4])` adds element-wise.
- [ ] `a.astype('float16')`, `a.byteswap()`, `a.tobytes()`, `a.dtype = 'uint16'` reinterpret as documented; `trailing_bits` reports leftovers.
- [ ] `Array('uint8', [1, 2]).equals(Array('uint8', [1, 2]))` is True; `pp()` prints the layout in `doc/array.rst`.
