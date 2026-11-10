- Typed path: a module-level `MISSING = object()` sentinel that is also told apart from None (or only compared with
  `is`/`==` and guarded before real use) now stays on the typed build as its own value, instead of sending the
  program to the dynamic build. +6 corpus programs typed. (DeepSeek-authored, Claude-reviewed.)
