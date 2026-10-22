# 01: Tracer — importable package with BitStore, Options, Dtype registry and exotic-float LUTs

**What to build:** Make `import bitstring` succeed and run one thin path end to end:
`Bits('0b101')`, `Bits(hex='ff')`, `Bits(uint=5, length=8)` construct; `.bin`, `.hex`, `.uint`,
`len()`, `==`, `str()`, `repr()` and slicing work; `Dtype('uint8')` resolves through the registry.

This ticket owns everything the import needs, in full where it is a small coupled unit:

- `bitstore.py`: implement the whole `BitStore` class (it is one coupled unit): `_copy`,
  `__getitem__`, `getindex`, `getslice`, `getslice_withstep`, `getslice_msb0`, `setall`,
  `setitem`/`delitem`-style mutators, `invert`, `reverse`, `count`, `any`/`all`,
  `find`/`rfind`/`findall_msb0`, `frombytes`/`tobytes`/`tobitarray`, `slice_to_*`
  conversions, `__len__` and LSB0 index translation (when `options.lsb0` is set, index 0 is
  the last bit). Immutable stores must refuse mutation.
- `bitstring_options.py`: `Options.set_lsb0` and properties `lsb0`, `bytealigned`,
  `mxfp_overflow` (only `'saturate'`/`'overflow'`, ValueError otherwise), `no_color`;
  switching lsb0 must rebind whatever `Bits`/`BitStore` methods depend on bit order.
- `dtypes.py`: all stubbed `Dtype` properties, `build`, `parse`, plus the deleted
  `Dtype._new_from_token`, `Dtype._create`, `AllowedLengths.only_one_value`,
  `DtypeDefinition.get_dtype`, `Register.add_dtype`, `add_dtype_alias`, `get_dtype`.
  Tokens like `'uint8'`, `'u8'`, `'uint:8'`, `'float16'`, `'bytes3'`, `'ue'` and
  `Dtype('e3m2mxfp', scale=2**6)` must resolve; bad names/lengths raise ValueError.
- `fp8.py`, `mxfp.py`: `Binary8Format` / `MXFPFormat` conversion methods, `create_luts`,
  module-level `decompress_luts` (imported by `__init__.py:74-75`). `luts.py` lacks
  `binary8_luts_compressed`: add it (run `python3 generate_luts.py` once `create_luts` works,
  or compute at import). Follow `doc/exotic_floats.rst` for rounding/saturation rules.
- `utils.py`: only the helpers `dtypes.py` needs to parse a token (e.g. a
  `parse_name_length_token(fmt, **kwargs) -> Tuple[str, Optional[int]]`). Leave
  `tokenparser`/`structparser`/`expand_brackets` to ticket 02.
- `bitstore_helpers.py`: implement `tidy_input_string`, `bin2bitstore`, `hex2bitstore`,
  `oct2bitstore` (with `_unsafe` variants as needed) and `uint2bitstore`; define every
  other converter the rest of the package will call (`int2bitstore`, `intle2bitstore`,
  `float2bitstore`, `floatle2bitstore`, `bfloat2bitstore`, `ue2bitstore`, `se2bitstore`,
  `uie2bitstore`, `sie2bitstore`, `bitstore_from_token`, exotic float converters...) with
  their final signatures — bodies may stay stubs for ticket 03. Write the signatures into
  a comment block at the top of the module so 03 and 04 agree.
- `bits.py`: define every `Bits` method `__init__.py:212-281` registers (`_setuintle`,
  `_setfloatbe`, `_getbits`, `_getpad`, `_getue`, `_sete4m3mxfp`...) so the module
  imports — stub bodies are fine. Implement the tracer path for real: `_initialise`,
  `_create_from_bitstype`, `_setauto_no_length_or_offset`/`_setauto` for single literal
  strings (`'0x..'`, `'0b..'`, `'0o..'`) and bytes, `_setbin_safe`, `_setbin_unsafe`,
  `_getbin`, `_sethex`, `_gethex`, `_setuint`, `_getuint`, `_getlength`, `_copy`, `_clear`,
  `_slice`, `_absolute_slice`, `_repr`, `_getbytes`/`tobytes`. Everything else in `Bits`
  belongs to ticket 04.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

**Files:** bitstring/bitstore.py, bitstring/bitstring_options.py, bitstring/dtypes.py, bitstring/fp8.py, bitstring/mxfp.py, bitstring/luts.py, bitstring/utils.py, bitstring/bitstore_helpers.py, bitstring/bits.py

**Context:**
`bitstring/__init__.py:64-79  imports + mxfp_decompress_luts()/binary8_decompress_luts()  — import order`
`bitstring/__init__.py:212-319  dtype_definitions / aliases / dtype_register.add_dtype(...)  — registry the tracer must satisfy`
`bitstring/bitstore.py:6  class BitStore(initializer, immutable=False)  — wraps bitarray; __slots__ _bitarray, modified_length, immutable`
`bitstring/bitstore_helpers.py:17  literal_bit_funcs  — maps '0x'/'0b'/'0o' to hex2bitstore/bin2bitstore/oct2bitstore`
`bitstring/bitstring_options.py:5  class Options  — singleton; __init__ calls self.set_lsb0(False)`
`bitstring/dtypes.py:32  Dtype.__new__(token, length=None, scale=None)  — calls _new_from_token / dtype_register.get_dtype`
`bitstring/dtypes.py:167  DtypeDefinition(name, set_fn, get_fn, ...)  — per-name definition; read_fn/get_fn wrappers exist`
`bitstring/dtypes.py:234  class Register  — singleton; names: Dict[str, DtypeDefinition]`
`bitstring/fp8.py:13  class Binary8Format(exp_bits, bias)  — float_to_int8, createLUT_for_binary8_to_float`
`bitstring/mxfp.py:9  class MXFPFormat(exp_bits, mantissa_bits, bias, mxfp_overflow)  — float_to_int, createLUT_*`
`generate_luts.py:32-45  mxfp.create_luts(); binary8.create_luts(); lut_* attributes  — expected LUT API`
`bitstring/bits.py:105  Bits.__new__ -> x._initialise(auto, length, offset, **kwargs)`
`bitstring/bits.py:176  Bits.__getitem__  — uses _bitstore.getindex / getslice_withstep`
`bitstring/bits.py:201-227  __str__/__repr__  — use _gethex, _getbin, _repr(name, length, pos)`
`scratch/missing_methods.py, scratch/missing_attrs.py  — list names referenced but undefined`

- [ ] `python3 -c "import bitstring"` exits 0.
- [ ] `Bits('0b101').bin == '101'`, `Bits(hex='ff').uint == 255`, `Bits('0xf')[0:2].bin == '11'`, `str(Bits('0x1f'))` is `'0x1f'`, `repr(Bits('0b1'))` is `"Bits('0b1')"`.
- [ ] `Dtype('uint8').length == 8`, `Dtype('u', 12).name == 'uint'`, `Dtype('float16').bitlength == 16`, `Dtype('ue').variable_length` is True, `Dtype('uint0x')` raises ValueError.
- [ ] `bitstring.options.lsb0 = True` makes `Bits('0b001')[0]` True; `options.mxfp_overflow = 'bad'` raises ValueError.
- [ ] `p4binary_fmt`/`e4m3mxfp_saturate_fmt` LUTs load: converting 1.0 to the int code and back via the LUT returns 1.0.
