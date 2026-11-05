import numpy as np
a0 = np.array([-4.0, 1e9, -42.12485])
a1 = np.array([14, 3])
a2 = np.array([[85.495126], [-47.95]])
a3 = np.array([73.464331, -25.521381])
np.set_printoptions(precision=2, suppress=False)
try:
    print('a2.flatten()', "=>", a2.flatten())
except Exception as ex:
    print('a2.flatten()', "EXC", type(ex).__name__)
try:
    print('np.max(a1, axis=0)', "=>", np.max(a1, axis=0))
except Exception as ex:
    print('np.max(a1, axis=0)', "EXC", type(ex).__name__)
try:
    print('a3.sum(axis=-1)', "=>", a3.sum(axis=-1))
except Exception as ex:
    print('a3.sum(axis=-1)', "EXC", type(ex).__name__)
try:
    print('a2 / 3', "=>", a2 / 3)
except Exception as ex:
    print('a2 / 3', "EXC", type(ex).__name__)
try:
    print('np.sign(a2)', "=>", np.sign(a2))
except Exception as ex:
    print('np.sign(a2)', "EXC", type(ex).__name__)
try:
    print('a3.min()', "=>", a3.min())
except Exception as ex:
    print('a3.min()', "EXC", type(ex).__name__)
try:
    print('np.diff(a3)', "=>", np.diff(a3))
except Exception as ex:
    print('np.diff(a3)', "EXC", type(ex).__name__)
try:
    print('np.array_equal(a2, a2)', "=>", np.array_equal(a2, a2))
except Exception as ex:
    print('np.array_equal(a2, a2)', "EXC", type(ex).__name__)
try:
    print('a0.std()', "=>", a0.std())
except Exception as ex:
    print('a0.std()', "EXC", type(ex).__name__)
try:
    print('a3.dtype', "=>", a3.dtype)
except Exception as ex:
    print('a3.dtype', "EXC", type(ex).__name__)
try:
    print('a3.astype(float)', "=>", a3.astype(float))
except Exception as ex:
    print('a3.astype(float)', "EXC", type(ex).__name__)
try:
    print('np.sign(a1)', "=>", np.sign(a1))
except Exception as ex:
    print('np.sign(a1)', "EXC", type(ex).__name__)
try:
    print('a1 * 2.5', "=>", a1 * 2.5)
except Exception as ex:
    print('a1 * 2.5', "EXC", type(ex).__name__)
try:
    print('a1.max()', "=>", a1.max())
except Exception as ex:
    print('a1.max()', "EXC", type(ex).__name__)
try:
    print('np.hstack([a2, a2])', "=>", np.hstack([a2, a2]))
except Exception as ex:
    print('np.hstack([a2, a2])', "EXC", type(ex).__name__)
try:
    print('np.expand_dims(a0, 0).shape', "=>", np.expand_dims(a0, 0).shape)
except Exception as ex:
    print('np.expand_dims(a0, 0).shape', "EXC", type(ex).__name__)
try:
    print('a1 - a2', "=>", a1 - a2)
except Exception as ex:
    print('a1 - a2', "EXC", type(ex).__name__)
try:
    print('np.expand_dims(a1, 0).shape', "=>", np.expand_dims(a1, 0).shape)
except Exception as ex:
    print('np.expand_dims(a1, 0).shape', "EXC", type(ex).__name__)
try:
    print('a1.all()', "=>", a1.all())
except Exception as ex:
    print('a1.all()', "EXC", type(ex).__name__)
try:
    print('[x for x in a0.ravel()]', "=>", [x for x in a0.ravel()])
except Exception as ex:
    print('[x for x in a0.ravel()]', "EXC", type(ex).__name__)
try:
    print('[x for x in a0.ravel()]', "=>", [x for x in a0.ravel()])
except Exception as ex:
    print('[x for x in a0.ravel()]', "EXC", type(ex).__name__)
try:
    print('np.hstack([a2, a2])', "=>", np.hstack([a2, a2]))
except Exception as ex:
    print('np.hstack([a2, a2])', "EXC", type(ex).__name__)
try:
    print('a2[0]', "=>", a2[0])
except Exception as ex:
    print('a2[0]', "EXC", type(ex).__name__)
try:
    print('a3 ** 2', "=>", a3 ** 2)
except Exception as ex:
    print('a3 ** 2', "EXC", type(ex).__name__)
try:
    print('a1 / 3', "=>", a1 / 3)
except Exception as ex:
    print('a1 / 3', "EXC", type(ex).__name__)
try:
    print('np.sqrt(np.abs(a1))', "=>", np.sqrt(np.abs(a1)))
except Exception as ex:
    print('np.sqrt(np.abs(a1))', "EXC", type(ex).__name__)
try:
    print('np.argsort(a1, kind="stable")', "=>", np.argsort(a1, kind="stable"))
except Exception as ex:
    print('np.argsort(a1, kind="stable")', "EXC", type(ex).__name__)
try:
    print('a0.std()', "=>", a0.std())
except Exception as ex:
    print('a0.std()', "EXC", type(ex).__name__)
try:
    print('float(a3.ravel()[0])', "=>", float(a3.ravel()[0]))
except Exception as ex:
    print('float(a3.ravel()[0])', "EXC", type(ex).__name__)
try:
    print('np.argsort(a2, kind="stable")', "=>", np.argsort(a2, kind="stable"))
except Exception as ex:
    print('np.argsort(a2, kind="stable")', "EXC", type(ex).__name__)
try:
    print('np.percentile(a0, 30)', "=>", np.percentile(a0, 30))
except Exception as ex:
    print('np.percentile(a0, 30)', "EXC", type(ex).__name__)
try:
    print('a3.T', "=>", a3.T)
except Exception as ex:
    print('a3.T', "EXC", type(ex).__name__)
try:
    print('a0.max()', "=>", a0.max())
except Exception as ex:
    print('a0.max()', "EXC", type(ex).__name__)
try:
    print('np.count_nonzero(a1)', "=>", np.count_nonzero(a1))
except Exception as ex:
    print('np.count_nonzero(a1)', "EXC", type(ex).__name__)
try:
    print('np.median(a0)', "=>", np.median(a0))
except Exception as ex:
    print('np.median(a0)', "EXC", type(ex).__name__)
try:
    print('a1.sum()', "=>", a1.sum())
except Exception as ex:
    print('a1.sum()', "EXC", type(ex).__name__)
try:
    print('a0 // a2', "=>", a0 // a2)
except Exception as ex:
    print('a0 // a2', "EXC", type(ex).__name__)
try:
    print('a3.argmax(axis=0)', "=>", a3.argmax(axis=0))
except Exception as ex:
    print('a3.argmax(axis=0)', "EXC", type(ex).__name__)
try:
    print('a1.argmax()', "=>", a1.argmax())
except Exception as ex:
    print('a1.argmax()', "EXC", type(ex).__name__)
try:
    print('a3.sum()', "=>", a3.sum())
except Exception as ex:
    print('a3.sum()', "EXC", type(ex).__name__)
try:
    print('a1.argmin()', "=>", a1.argmin())
except Exception as ex:
    print('a1.argmin()', "EXC", type(ex).__name__)
try:
    print('np.where(a1 > 0, a1, a3)', "=>", np.where(a1 > 0, a1, a3))
except Exception as ex:
    print('np.where(a1 > 0, a1, a3)', "EXC", type(ex).__name__)
try:
    print('np.sort(a1, axis=None)', "=>", np.sort(a1, axis=None))
except Exception as ex:
    print('np.sort(a1, axis=None)', "EXC", type(ex).__name__)
try:
    print('repr(a0)', "=>", repr(a0))
except Exception as ex:
    print('repr(a0)', "EXC", type(ex).__name__)
try:
    print('a2.astype(float)', "=>", a2.astype(float))
except Exception as ex:
    print('a2.astype(float)', "EXC", type(ex).__name__)
try:
    print('a1.sum()', "=>", a1.sum())
except Exception as ex:
    print('a1.sum()', "EXC", type(ex).__name__)
try:
    print('np.stack([a2, a2], axis=1)', "=>", np.stack([a2, a2], axis=1))
except Exception as ex:
    print('np.stack([a2, a2], axis=1)', "EXC", type(ex).__name__)
try:
    print('np.sort(a0)', "=>", np.sort(a0))
except Exception as ex:
    print('np.sort(a0)', "EXC", type(ex).__name__)
try:
    print('a0 * 2.5', "=>", a0 * 2.5)
except Exception as ex:
    print('a0 * 2.5', "EXC", type(ex).__name__)
try:
    print('np.log(np.abs(a0) + 1)', "=>", np.log(np.abs(a0) + 1))
except Exception as ex:
    print('np.log(np.abs(a0) + 1)', "EXC", type(ex).__name__)
try:
    print('np.percentile(a0, 30)', "=>", np.percentile(a0, 30))
except Exception as ex:
    print('np.percentile(a0, 30)', "EXC", type(ex).__name__)
try:
    print('a1 ** 2', "=>", a1 ** 2)
except Exception as ex:
    print('a1 ** 2', "EXC", type(ex).__name__)
try:
    print('np.array_equal(a3, a2)', "=>", np.array_equal(a3, a2))
except Exception as ex:
    print('np.array_equal(a3, a2)', "EXC", type(ex).__name__)
try:
    print('a3 * 2.5', "=>", a3 * 2.5)
except Exception as ex:
    print('a3 * 2.5', "EXC", type(ex).__name__)
try:
    print('-a2', "=>", -a2)
except Exception as ex:
    print('-a2', "EXC", type(ex).__name__)
try:
    print('a3 + 1', "=>", a3 + 1)
except Exception as ex:
    print('a3 + 1', "EXC", type(ex).__name__)
try:
    print('abs(a1)', "=>", abs(a1))
except Exception as ex:
    print('abs(a1)', "EXC", type(ex).__name__)
try:
    print('a1 % 4', "=>", a1 % 4)
except Exception as ex:
    print('a1 % 4', "EXC", type(ex).__name__)
try:
    print('np.ones_like(a0, dtype=float)', "=>", np.ones_like(a0, dtype=float))
except Exception as ex:
    print('np.ones_like(a0, dtype=float)', "EXC", type(ex).__name__)
try:
    print('int(a3.ravel()[0])', "=>", int(a3.ravel()[0]))
except Exception as ex:
    print('int(a3.ravel()[0])', "EXC", type(ex).__name__)
m = a0.copy()
try:
    m[0] = 7
    m[m > 3] = 0
    m += 1
    print(m)
except Exception as ex:
    print("EXC", type(ex).__name__)
