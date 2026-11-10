- Native numpy: `argsort()` with the default kind on arrays with repeated values (int64, int32, float64, float32)
  now gives numpy's exact order instead of refusing. It follows numpy's own CPU path (x86-simd-sort on AVX-512 or
  AVX2, introsort otherwise) and honours `NPY_DISABLE_CPU_FEATURES`; ties off x86, and the few cases numpy hands to
  std::sort, still refuse. (DeepSeek-authored, Claude-reviewed.)
