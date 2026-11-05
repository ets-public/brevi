import numpy as np
a0 = np.array([-12, 3, 18, 10, 20])
a1 = np.array([[[-4], [15], [-6], [-8], [10]]])
a2 = np.array([[[True, True, True, False], [True, True, True, True], [False, True, False, False], [False, False, False, False]], [[True, False, True, True], [True, False, True, False], [False, False, False, False], [False, False, True, False]], [[True, False, True, False], [True, True, False, False], [True, True, False, False], [True, False, True, False]], [[True, True, False, False], [False, True, True, True], [False, False, False, True], [True, False, True, True]], [[True, True, True, False], [False, False, True, True], [False, False, False, True], [False, False, False, False]]])
a3 = np.array([[[-3, 7, 20, -5, -1]], [[7, -4, 13, -1, 15]], [[1, -20, 6, 17, 0]], [[-19, 4, 19, 17, 20]], [[-12, -17, 20, 20, 1]]])
np.set_printoptions(precision=2, suppress=True)
try:
    print('a3 > 2', "=>", a3 > 2)
except Exception as ex:
    print('a3 > 2', "EXC", type(ex).__name__)
try:
    print('[x for x in a2.ravel()]', "=>", [x for x in a2.ravel()])
except Exception as ex:
    print('[x for x in a2.ravel()]', "EXC", type(ex).__name__)
try:
    print('np.percentile(a3, 30)', "=>", np.percentile(a3, 30))
except Exception as ex:
    print('np.percentile(a3, 30)', "EXC", type(ex).__name__)
try:
    print('int(a0.ravel()[0])', "=>", int(a0.ravel()[0]))
except Exception as ex:
    print('int(a0.ravel()[0])', "EXC", type(ex).__name__)
try:
    print('a2 > 2', "=>", a2 > 2)
except Exception as ex:
    print('a2 > 2', "EXC", type(ex).__name__)
try:
    print('float(a2.ravel()[0])', "=>", float(a2.ravel()[0]))
except Exception as ex:
    print('float(a2.ravel()[0])', "EXC", type(ex).__name__)
try:
    print('a1 + a2', "=>", a1 + a2)
except Exception as ex:
    print('a1 + a2', "EXC", type(ex).__name__)
try:
    print('a2 > 2', "=>", a2 > 2)
except Exception as ex:
    print('a2 > 2', "EXC", type(ex).__name__)
try:
    print('a3 / a0', "=>", a3 / a0)
except Exception as ex:
    print('a3 / a0', "EXC", type(ex).__name__)
try:
    print('np.ones_like(a0, dtype=float)', "=>", np.ones_like(a0, dtype=float))
except Exception as ex:
    print('np.ones_like(a0, dtype=float)', "EXC", type(ex).__name__)
try:
    print('np.roll(a1, 1)', "=>", np.roll(a1, 1))
except Exception as ex:
    print('np.roll(a1, 1)', "EXC", type(ex).__name__)
try:
    print('a2 > 2', "=>", a2 > 2)
except Exception as ex:
    print('a2 > 2', "EXC", type(ex).__name__)
try:
    print('a3.std(axis=0)', "=>", a3.std(axis=0))
except Exception as ex:
    print('a3.std(axis=0)', "EXC", type(ex).__name__)
try:
    print('a2 ** 2', "=>", a2 ** 2)
except Exception as ex:
    print('a2 ** 2', "EXC", type(ex).__name__)
try:
    print('np.allclose(a1, a3)', "=>", np.allclose(a1, a3))
except Exception as ex:
    print('np.allclose(a1, a3)', "EXC", type(ex).__name__)
try:
    print('np.tile(a1, 2)', "=>", np.tile(a1, 2))
except Exception as ex:
    print('np.tile(a1, 2)', "EXC", type(ex).__name__)
try:
    print('np.nansum(a1)', "=>", np.nansum(a1))
except Exception as ex:
    print('np.nansum(a1)', "EXC", type(ex).__name__)
try:
    print('np.sign(a1)', "=>", np.sign(a1))
except Exception as ex:
    print('np.sign(a1)', "EXC", type(ex).__name__)
try:
    print('np.isclose(a1, a2)', "=>", np.isclose(a1, a2))
except Exception as ex:
    print('np.isclose(a1, a2)', "EXC", type(ex).__name__)
try:
    print('np.repeat(a1, 2)', "=>", np.repeat(a1, 2))
except Exception as ex:
    print('np.repeat(a1, 2)', "EXC", type(ex).__name__)
try:
    print('np.percentile(a0, 30)', "=>", np.percentile(a0, 30))
except Exception as ex:
    print('np.percentile(a0, 30)', "EXC", type(ex).__name__)
try:
    print('a1 / 3', "=>", a1 / 3)
except Exception as ex:
    print('a1 / 3', "EXC", type(ex).__name__)
try:
    print('sorted(a2.ravel())', "=>", sorted(a2.ravel()))
except Exception as ex:
    print('sorted(a2.ravel())', "EXC", type(ex).__name__)
try:
    print('a3.mean()', "=>", a3.mean())
except Exception as ex:
    print('a3.mean()', "EXC", type(ex).__name__)
try:
    print('np.sign(a3)', "=>", np.sign(a3))
except Exception as ex:
    print('np.sign(a3)', "EXC", type(ex).__name__)
try:
    print('np.exp(a1 / 50)', "=>", np.exp(a1 / 50))
except Exception as ex:
    print('np.exp(a1 / 50)', "EXC", type(ex).__name__)
try:
    print('a3[np.newaxis]', "=>", a3[np.newaxis])
except Exception as ex:
    print('a3[np.newaxis]', "EXC", type(ex).__name__)
try:
    print('float(a1.ravel()[0])', "=>", float(a1.ravel()[0]))
except Exception as ex:
    print('float(a1.ravel()[0])', "EXC", type(ex).__name__)
try:
    print('a3[::-1]', "=>", a3[::-1])
except Exception as ex:
    print('a3[::-1]', "EXC", type(ex).__name__)
try:
    print('a2.prod()', "=>", a2.prod())
except Exception as ex:
    print('a2.prod()', "EXC", type(ex).__name__)
try:
    print('np.allclose(a2, a0)', "=>", np.allclose(a2, a0))
except Exception as ex:
    print('np.allclose(a2, a0)', "EXC", type(ex).__name__)
try:
    print('np.sign(a1)', "=>", np.sign(a1))
except Exception as ex:
    print('np.sign(a1)', "EXC", type(ex).__name__)
try:
    print('np.unique(a1)', "=>", np.unique(a1))
except Exception as ex:
    print('np.unique(a1)', "EXC", type(ex).__name__)
try:
    print('a0.std(axis=0)', "=>", a0.std(axis=0))
except Exception as ex:
    print('a0.std(axis=0)', "EXC", type(ex).__name__)
try:
    print('a0.reshape(-1)', "=>", a0.reshape(-1))
except Exception as ex:
    print('a0.reshape(-1)', "EXC", type(ex).__name__)
try:
    print('a2 // 3', "=>", a2 // 3)
except Exception as ex:
    print('a2 // 3', "EXC", type(ex).__name__)
try:
    print('a0.min()', "=>", a0.min())
except Exception as ex:
    print('a0.min()', "EXC", type(ex).__name__)
try:
    print('a0.flatten()', "=>", a0.flatten())
except Exception as ex:
    print('a0.flatten()', "EXC", type(ex).__name__)
try:
    print('np.allclose(a2, a3)', "=>", np.allclose(a2, a3))
except Exception as ex:
    print('np.allclose(a2, a3)', "EXC", type(ex).__name__)
try:
    print('np.where(a0 > 1, a0, -1)', "=>", np.where(a0 > 1, a0, -1))
except Exception as ex:
    print('np.where(a0 > 1, a0, -1)', "EXC", type(ex).__name__)
try:
    print('a1[0]', "=>", a1[0])
except Exception as ex:
    print('a1[0]', "EXC", type(ex).__name__)
try:
    print('np.roll(a0, 1)', "=>", np.roll(a0, 1))
except Exception as ex:
    print('np.roll(a0, 1)', "EXC", type(ex).__name__)
try:
    print('a2 & a1', "=>", a2 & a1)
except Exception as ex:
    print('a2 & a1', "EXC", type(ex).__name__)
try:
    print('np.max(a0, axis=0)', "=>", np.max(a0, axis=0))
except Exception as ex:
    print('np.max(a0, axis=0)', "EXC", type(ex).__name__)
try:
    print('f"{a3.ravel()[0]:.3f}"', "=>", f"{a3.ravel()[0]:.3f}")
except Exception as ex:
    print('f"{a3.ravel()[0]:.3f}"', "EXC", type(ex).__name__)
try:
    print('np.isnan(a2)', "=>", np.isnan(a2))
except Exception as ex:
    print('np.isnan(a2)', "EXC", type(ex).__name__)
try:
    print('a3.max(axis=-1)', "=>", a3.max(axis=-1))
except Exception as ex:
    print('a3.max(axis=-1)', "EXC", type(ex).__name__)
try:
    print('a2.max()', "=>", a2.max())
except Exception as ex:
    print('a2.max()', "EXC", type(ex).__name__)
try:
    print('a1.reshape(-1)', "=>", a1.reshape(-1))
except Exception as ex:
    print('a1.reshape(-1)', "EXC", type(ex).__name__)
try:
    print('a0.std(axis=0)', "=>", a0.std(axis=0))
except Exception as ex:
    print('a0.std(axis=0)', "EXC", type(ex).__name__)
try:
    print('a0.shape', "=>", a0.shape)
except Exception as ex:
    print('a0.shape', "EXC", type(ex).__name__)
try:
    print('a3[-1]', "=>", a3[-1])
except Exception as ex:
    print('a3[-1]', "EXC", type(ex).__name__)
try:
    print('max(a2.ravel())', "=>", max(a2.ravel()))
except Exception as ex:
    print('max(a2.ravel())', "EXC", type(ex).__name__)
try:
    print('np.percentile(a3, 30)', "=>", np.percentile(a3, 30))
except Exception as ex:
    print('np.percentile(a3, 30)', "EXC", type(ex).__name__)
try:
    print('a2 == a2', "=>", a2 == a2)
except Exception as ex:
    print('a2 == a2', "EXC", type(ex).__name__)
try:
    print('a1.var(ddof=1)', "=>", a1.var(ddof=1))
except Exception as ex:
    print('a1.var(ddof=1)', "EXC", type(ex).__name__)
try:
    print('a0[a0 > 0]', "=>", a0[a0 > 0])
except Exception as ex:
    print('a0[a0 > 0]', "EXC", type(ex).__name__)
try:
    print('np.minimum(a0, a3)', "=>", np.minimum(a0, a3))
except Exception as ex:
    print('np.minimum(a0, a3)', "EXC", type(ex).__name__)
try:
    print('a0.mean()', "=>", a0.mean())
except Exception as ex:
    print('a0.mean()', "EXC", type(ex).__name__)
try:
    print('np.nonzero(a2)', "=>", np.nonzero(a2))
except Exception as ex:
    print('np.nonzero(a2)', "EXC", type(ex).__name__)
m = a0.copy()
try:
    m[0] = 7
    m[m > 3] = 0
    m += 1
    print(m)
except Exception as ex:
    print("EXC", type(ex).__name__)
