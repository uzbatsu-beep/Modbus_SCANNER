# Typed data / Декодирование

Target specification for R01/R05. Existing decoder remains the foundation; do not fork decoding logic into UI.

| Type family | Representation |
|---|---|
| BOOL, BIT | coils or selected bit in register, explicit bit index |
| SINT/USINT, BYTE | selected byte with explicit byte position |
| INT/UINT, WORD | 16 bits |
| DINT/UDINT, DWORD, REAL | 32 bits; explicit byte/word order |
| LINT/ULINT, LWORD, LREAL | 64 bits; explicit supported permutations |
| STRING/ASCII | explicit word count, byte order, terminator policy and encoding |
| ENUM | integer base type + value-to-label dictionary |
| BITFIELD | named bit/range definitions, masks, unknown bits visible |
| BCD | explicit digit count/sign convention; reject invalid nibbles |
| Fixed point | explicit multiplier/divider/offset; reject zero divider and non-finite results |

Keep raw words unchanged. Decode order: validate length → reorder bytes/words → interpret type → apply declared scaling → format (never round internal state just for display). Exact 64-bit integer values cross JSON IPC as decimal strings; use BigInt for exact manipulation, document lossy graph conversion if necessary. Do not coerce unknown enum values or invalid IEEE float to zero.

Tests: 0xEC78 signed = -5000, scaled by 0.01 = -50.00; unsigned = 60536. Include signed extrema, endian permutations, NaN/Inf, malformed lengths, uint64 max and unknown enum labels. Trends and min/max/average use engineering values; quality gaps are visible, not connected as successful samples. No device-specific ENUM definitions hard-coded in core. See [DATA_MODEL](DATA_MODEL.md).
