- `0 ** -1`, `0.0 ** -2.5`, `pow(0, -1)` and `x **= -1` with a zero base now raise CPython's
  `ZeroDivisionError("0.0 cannot be raised to a negative power")` on every build instead of printing `inf`, and
  the error can be caught on the typed build. `pow(a, -1, m)` returns the modular inverse, or raises
  `ValueError("base is not invertible for the given modulus")`. Found by the fuzz harness. (DeepSeek-authored,
  Claude-reviewed.)
