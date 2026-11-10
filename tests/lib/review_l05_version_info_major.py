import sys
vi = sys.version_info
print(vi[:2])
print(vi >= (3,8))
print(vi.major)
print(vi.minor, vi.releaselevel, vi.serial)
print(len(vi), vi[0], vi[1], vi[3], vi[4], vi[-1])
print(vi.index('final'), tuple(vi)[3:])
print(vi.n_fields, vi.__match_args__)
print(vi > (3, 12, 99), vi == (3, 13), (2, 99) < vi)
print(hasattr(vi, 'major'), hasattr(vi, 'minor'), hasattr(vi, 'micro'), hasattr(vi, 'releaselevel'), hasattr(vi, 'serial'), hasattr(vi, '_fields'), hasattr(vi, '_asdict'), hasattr(vi, '_replace'))
print(isinstance(vi, tuple), sys.version_info is vi)
print(vi[:2] == (3, 13))
print(hash(vi) == hash(tuple(vi)), tuple(vi) in {vi})
