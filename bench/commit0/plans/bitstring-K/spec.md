# Spec: rebuild the `bitstring` library (slug: bench)

## Outcome

`import bitstring` works and the package behaves as documented in `spec.pdf`, `README.md`
and `doc/*.rst` (bitstring 4.2.x API): `Bits`, `BitArray`, `ConstBitStream`, `BitStream`,
`Array`, `Dtype`, `pack`, `options`, the exceptions, the exotic float formats
(`p3binary`, `p4binary`, the MXFP formats, `bfloat`), exp-Golomb codes, `pp()`
pretty printing, `python -m bitstring`, and LSB0 mode.

A hidden test suite (the upstream bitstring tests) grades the result.

## Current state (what the stubbing did)

The repo is not just "bodies replaced by `pass`". Whole functions and methods were
deleted too, so today `import bitstring` fails with
`NameError: name 'hex2bitstore' is not defined` (bitstore_helpers.py:17).
Names referenced but no longer defined include:

- `bitstore.py`: almost all of `BitStore` (getindex, getslice, getslice_withstep, setall,
  setitem/ delitem / invert / reverse / count / any / all / find / rfind / findall_msb0,
  frombytes / tobytes / slice_to_*, `__getitem__`, LSB0 index translation, etc.).
- `bitstore_helpers.py`: every converter (`bin2bitstore`, `hex2bitstore`, `oct2bitstore`,
  `int2bitstore`, `float2bitstore`, `ue/se/uie/sie2bitstore`, `bitstore_from_token`, the
  exotic float converters...).
- `bitstring_options.py`: `Options.set_lsb0`, `lsb0`/`bytealigned`/`mxfp_overflow`/`no_color`
  properties and validation.
- `dtypes.py`: `Dtype._new_from_token`, `Dtype._create`, `AllowedLengths.only_one_value`,
  `Register.add_dtype`, `add_dtype_alias`, `get_dtype`, `DtypeDefinition.get_dtype`.
- `fp8.py` / `mxfp.py`: `decompress_luts`, `create_luts`, LUT-based conversion helpers;
  `luts.py` lacks `binary8_luts_compressed` (generate it with `generate_luts.py` once
  `create_luts` works, or compute the LUTs at import).
- `utils.py`: helpers such as `parse_name_length_token`, `parse_single_struct_token`,
  `preprocess_tokens` (only called from stubbed bodies, so not visible to a static scan).
- `bits.py`: `_initialise`, `_create_from_bitstype`, `_repr`, all `_set*/_get*` dtype methods
  that `__init__.py:212-281` registers (`_setuintle`, `_setfloatbe`, `_sete4m3mxfp`,
  `_getbits`, `_getpad`, `_getue`...), plus others.
- `bitarray_.py`: `_setitem_int`, `_setitem_slice`, `_append`, `_prepend`...
- `array_.py`: `_set_dtype`, `dtype`/`itemsize`/`trailing_bits` properties, `extend`,
  `append`, `reverse`, `fromfile`, `tolist`, `_apply_op_between_arrays`, `_eq_ne`,
  `_calculate_auto_scale`...

`python3 scratch/missing_methods.py bitstring` and `python3 scratch/missing_attrs.py bitstring`
list attribute/import names referenced but not defined (they cannot see names used only
inside stubbed bodies).

## Constraints

- Keep every public name, signature and module layout unchanged. Re-add deleted
  private helpers with the names the surviving code calls.
- `bitarray` is the storage backend (`BitStore` wraps `bitarray.bitarray`).
- Python ≥ 3.8 semantics; the repo's existing f-strings already need 3.12 (`dtypes.py:253`).
- Tests are hidden: each worker writes its own checks under `scratch/` and runs them with
  one-line `python3 ...` commands. Doc examples in `doc/*.rst` and `README.md` are the
  best oracle; `spec.pdf` is the reference manual.
- One command per shell call; no `;`, `&&`, `|`, heredocs or redirection.
- Dispatch is `serial` on Files overlap; tickets after the tracer touch disjoint files
  except the integration ticket.

## Non-goals

- No new public API, no changes to `doc/`, `README.md`, `pyproject.toml`.
- No performance work beyond what the docs promise.
- No committed test suite (checks live in `scratch/`).
