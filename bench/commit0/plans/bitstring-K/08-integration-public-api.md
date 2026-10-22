# 08: Integration — whole public API end to end, fix the seams

**What to build:** Exercise the complete public API as documented and fix whatever breaks
between tickets. Write `scratch/integration_*.py` scripts that run the examples from
`README.md`, `doc/introduction.rst`, `doc/walkthrough.ipynb` (code cells), `doc/bits.rst`,
`doc/bitarray.rst`, `doc/constbitstream.rst`, `doc/bitstream.rst`, `doc/array.rst`,
`doc/dtypes.rst`, `doc/exotic_floats.rst`, `doc/exp-golomb.rst`, `doc/functions.rst`,
`doc/quick_reference.rst` and `doc/optimisation.rst`, assert the documented results, and fix
the source until they pass. Then:

- re-run `python3 scratch/missing_methods.py bitstring` and
  `python3 scratch/missing_attrs.py bitstring`: no referenced name may be undefined.
- grep for any remaining `pass`-only stub bodies in `bitstring/` and implement them.
- check `python3 -m bitstring int:16=-400` prints `0xfe70` and the other `__main__.py`
  examples.
- check LSB0 mode, `options.bytealigned`, `options.mxfp_overflow`, copy/deepcopy, pickling,
  hashing, and that `Bits` stays immutable (mutation raises) across all classes.
- check docstring generation in `__init__.py:321-332` ran (no `[GENERATED_PROPERTY_DESCRIPTIONS]` left).

**Blocked by:** 01, 02, 03, 04, 05, 06, 07

**Status:** ready-for-agent

**Files:** bitstring/__init__.py, bitstring/__main__.py, bitstring/array_.py, bitstring/bitarray_.py, bitstring/bits.py, bitstring/bitstore.py, bitstring/bitstore_helpers.py, bitstring/bitstream.py, bitstring/bitstring_options.py, bitstring/dtypes.py, bitstring/exceptions.py, bitstring/fp8.py, bitstring/luts.py, bitstring/methods.py, bitstring/mxfp.py, bitstring/utils.py

**Context:**
`bitstring/__init__.py:64-82  public imports, LUT decompression, options singleton`
`bitstring/__init__.py:212-319  dtype registry population`
`bitstring/__init__.py:335  __all__  — public names`
`bitstring/__main__.py:8  main()  — CLI`
`bitstring/bits.py:25  Bits`
`bitstring/bitarray_.py:12  BitArray(Bits)`
`bitstring/bitstream.py:10  ConstBitStream(Bits); :385 BitStream(ConstBitStream, BitArray)`
`bitstring/array_.py:20  Array`
`bitstring/dtypes.py:9  Dtype; :234 Register`
`bitstring/methods.py:10  pack`
`bitstring/bitstring_options.py:5  Options`
`scratch/missing_methods.py, scratch/missing_attrs.py  — undefined-name scanners`

- [ ] Every README and doc example scripted under `scratch/` produces its documented output.
- [ ] Both scanner scripts report nothing and no `pass`-only function body remains in `bitstring/`.
- [ ] `python3 -m bitstring int:16=-400` prints `0xfe70`; `python3 -m bitstring float:32=0.2 bin` prints `00111110010011001100110011001101`.
- [ ] `Bits` instances refuse mutation; `copy.copy`, `copy.deepcopy`, `pickle` round-trip all five classes.
