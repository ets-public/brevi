- `chr()` of an int outside 0..0x10FFFF now raises CPython's `ValueError("chr() arg not in range(0x110000)")` on
  every build, including ints past 64 bits (it was IndexError or OverflowError) and negative ints on the typed
  build (which returned ""); the typed build can catch it. A lone surrogate (U+D800..U+DFFF) stops as unsupported
  instead of giving a different string. Found by the fuzz harness. (DeepSeek-authored, Claude-reviewed.)
