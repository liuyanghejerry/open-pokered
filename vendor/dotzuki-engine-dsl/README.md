# Vendored dotzuki-engine-dsl compatibility patch

Source: dotzuki v0.10.0 (`c68c501ed8100b660ca40ce0ececa648897a36dc`).

The only source change replaces `f64::fract()` in `core_host::integer_arg`
with an integer round-trip check. `fract()` is unavailable from `core` in the
GBA `no_std` build. Accepted integer arguments are already range-limited to
`u8`, `u16`, or `i16`; values outside those ranges are rejected by the same
validation function. The upstream source and both upstream license files are
retained.
