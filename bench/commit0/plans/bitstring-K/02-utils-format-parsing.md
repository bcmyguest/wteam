# 02: utils — format-string tokenizer, struct codes and bracket expansion

**What to build:** The format-string parser used by `Bits('uint:8=3, 0xff')`, `pack`,
`unpack`, `readlist` and `Array('>H')`. Implement in `bitstring/utils.py`:

- `expand_brackets(s)`: `'2*(a, b)'` → `'a,b,a,b'` (nested brackets, whitespace tolerant;
  unbalanced brackets raise ValueError).
- `structparser(m)`: struct-style tokens like `'>2hL'` → `['intbe:16', 'intbe:16', 'uintbe:32']`
  using `REPLACEMENTS_BE/LE/NE`; `'@'`/`'='` mean native.
- `tokenparser(fmt, keys=())` → `(stretchy, [(name, length, value), ...])`: handles
  `'uint:8=3'`, `'u8'`, `'hex=ff'`, `'0xff'`, `'bits'` (stretchy, length None), `'3*int:4'`
  multiplicative tokens, `'pad:3'`, keyword tokens in `keys` passed through untouched,
  struct codes, `bytes:3` lengths in bytes. Raise ValueError for malformed tokens.
- Re-add the deleted helpers other modules call with these signatures (keep whatever
  ticket 01 already wrote for `parse_name_length_token`):
  `parse_single_struct_token(fmt) -> Optional[Tuple[str, Optional[int]]]`,
  `preprocess_tokens(fmt) -> List[str]` (split on commas after bracket expansion, expand
  `n*token` and struct codes), `parse_name_length_token(fmt, **kwargs)`.

Documented grammar: `doc/interpretation.rst`, `doc/functions.rst` (pack), `doc/quick_reference.rst`.

**Blocked by:** 01

**Status:** ready-for-agent

**Files:** bitstring/utils.py

**Context:**
`bitstring/utils.py:5-18  NAME_INT_RE, NAME_KWARG_RE, DEFAULT_BITS, MULTIPLICATIVE_RE, LITERAL_RE, STRUCT_PACK_RE, REPLACEMENTS_*, PACK_CODE_SIZE  — regexes/tables to use`
`bitstring/utils.py:20  structparser(m: Match[str]) -> List[str]`
`bitstring/utils.py:24  @lru_cache tokenparser(fmt: str, keys: Tuple[str, ...]=()) -> Tuple[bool, List[Tuple[str, Union[int,str,None], Optional[str]]]]`
`bitstring/utils.py:41  expand_brackets(s: str) -> str`
`bitstring/dtypes.py:9  Dtype  — tokenparser may consult dtype_register.names for valid names`
`bitstring/methods.py:10  pack(fmt, *values, **kwargs)  — caller (ticket 06)`
`bitstring/bits.py:666  Bits.unpack  — caller (ticket 04)`

- [ ] `expand_brackets('2*(a, b), c') == 'a,b,a,b,c'` (whitespace stripped as upstream does) and nested brackets expand.
- [ ] `tokenparser('uint:8=3, hex=ff')` returns `(False, [('uint', 8, '3'), ('hex', None, 'ff')])`-shaped output.
- [ ] `tokenparser('bits, uint:4')[0]` is True (stretchy token).
- [ ] `tokenparser('3*u8')` yields three uint/8 tokens; `tokenparser('>hL')` yields intbe:16 then uintbe:32.
- [ ] `tokenparser('a', ('a',))` passes keyword `a` through.
