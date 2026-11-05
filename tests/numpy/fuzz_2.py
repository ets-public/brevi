import numpy as np
a0 = np.array([-66.183277])
a1 = np.array([[16.2, 27.694268, 98.96, -11.03, 74.21], [-36.307, 76.8, -53.0, -65.2822, 99.73671], [99.036, 46.880338, 18.7, 92.4, -20.0273]])
a2 = np.array([[-3, 11], [12, 12], [2, 9], [9, 2], [16, 15]])
a3 = np.array([[67.0053, 82.28], [59.851776, 3.54955], [-18.66589, 2.37833], [-84.924926, -98.315635]])
np.set_printoptions(precision=2, suppress=False)
try:
    print('a1.argmax()', "=>", a1.argmax())
except Exception as ex:
    print('a1.argmax()', "EXC", type(ex).__name__)
try:
    print('np.percentile(a0, [10, 50, 90])', "=>", np.percentile(a0, [10, 50, 90]))
except Exception as ex:
    print('np.percentile(a0, [10, 50, 90])', "EXC", type(ex).__name__)
try:
    print('f"{a1.ravel()[0]:.3f}"', "=>", f"{a1.ravel()[0]:.3f}")
except Exception as ex:
    print('f"{a1.ravel()[0]:.3f}"', "EXC", type(ex).__name__)
try:
    print('np.mean(a1)', "=>", np.mean(a1))
except Exception as ex:
    print('np.mean(a1)', "EXC", type(ex).__name__)
try:
    print('a1.std()', "=>", a1.std())
except Exception as ex:
    print('a1.std()', "EXC", type(ex).__name__)
try:
    print('{"v": a0.ravel()[0]}', "=>", {"v": a0.ravel()[0]})
except Exception as ex:
    print('{"v": a0.ravel()[0]}', "EXC", type(ex).__name__)
try:
    print('np.max(a2, axis=0)', "=>", np.max(a2, axis=0))
except Exception as ex:
    print('np.max(a2, axis=0)', "EXC", type(ex).__name__)
try:
    print('a0.T', "=>", a0.T)
except Exception as ex:
    print('a0.T', "EXC", type(ex).__name__)
try:
    print('a0.mean()', "=>", a0.mean())
except Exception as ex:
    print('a0.mean()', "EXC", type(ex).__name__)
try:
    print('repr(a0)', "=>", repr(a0))
except Exception as ex:
    print('repr(a0)', "EXC", type(ex).__name__)
try:
    print('[x for x in a1.ravel()]', "=>", [x for x in a1.ravel()])
except Exception as ex:
    print('[x for x in a1.ravel()]', "EXC", type(ex).__name__)
try:
    print('np.exp(a1 / 50)', "=>", np.exp(a1 / 50))
except Exception as ex:
    print('np.exp(a1 / 50)', "EXC", type(ex).__name__)
try:
    print('a0.astype(bool)', "=>", a0.astype(bool))
except Exception as ex:
    print('a0.astype(bool)', "EXC", type(ex).__name__)
try:
    print('sorted(a0.ravel())', "=>", sorted(a0.ravel()))
except Exception as ex:
    print('sorted(a0.ravel())', "EXC", type(ex).__name__)
try:
    print('np.tile(a0, 2)', "=>", np.tile(a0, 2))
except Exception as ex:
    print('np.tile(a0, 2)', "EXC", type(ex).__name__)
try:
    print('np.zeros_like(a3)', "=>", np.zeros_like(a3))
except Exception as ex:
    print('np.zeros_like(a3)', "EXC", type(ex).__name__)
try:
    print('a3 == a3', "=>", a3 == a3)
except Exception as ex:
    print('a3 == a3', "EXC", type(ex).__name__)
try:
    print('a3.max(axis=-1)', "=>", a3.max(axis=-1))
except Exception as ex:
    print('a3.max(axis=-1)', "EXC", type(ex).__name__)
try:
    print('a1 // 3', "=>", a1 // 3)
except Exception as ex:
    print('a1 // 3', "EXC", type(ex).__name__)
try:
    print('np.round(a2, 1)', "=>", np.round(a2, 1))
except Exception as ex:
    print('np.round(a2, 1)', "EXC", type(ex).__name__)
try:
    print('a3.dtype', "=>", a3.dtype)
except Exception as ex:
    print('a3.dtype', "EXC", type(ex).__name__)
try:
    print('a3.shape', "=>", a3.shape)
except Exception as ex:
    print('a3.shape', "EXC", type(ex).__name__)
try:
    print('np.repeat(a2, 2)', "=>", np.repeat(a2, 2))
except Exception as ex:
    print('np.repeat(a2, 2)', "EXC", type(ex).__name__)
try:
    print('np.count_nonzero(a2)', "=>", np.count_nonzero(a2))
except Exception as ex:
    print('np.count_nonzero(a2)', "EXC", type(ex).__name__)
try:
    print('np.hstack([a3, a3])', "=>", np.hstack([a3, a3]))
except Exception as ex:
    print('np.hstack([a3, a3])', "EXC", type(ex).__name__)
try:
    print('list(a1)', "=>", list(a1))
except Exception as ex:
    print('list(a1)', "EXC", type(ex).__name__)
try:
    print('a0[::2]', "=>", a0[::2])
except Exception as ex:
    print('a0[::2]', "EXC", type(ex).__name__)
try:
    print('a1.any()', "=>", a1.any())
except Exception as ex:
    print('a1.any()', "EXC", type(ex).__name__)
try:
    print('np.max(a1, axis=0)', "=>", np.max(a1, axis=0))
except Exception as ex:
    print('np.max(a1, axis=0)', "EXC", type(ex).__name__)
try:
    print('a1.cumsum()', "=>", a1.cumsum())
except Exception as ex:
    print('a1.cumsum()', "EXC", type(ex).__name__)
try:
    print('np.percentile(a2, [10, 50, 90])', "=>", np.percentile(a2, [10, 50, 90]))
except Exception as ex:
    print('np.percentile(a2, [10, 50, 90])', "EXC", type(ex).__name__)
try:
    print('repr(a1)', "=>", repr(a1))
except Exception as ex:
    print('repr(a1)', "EXC", type(ex).__name__)
try:
    print('a3.astype(float)', "=>", a3.astype(float))
except Exception as ex:
    print('a3.astype(float)', "EXC", type(ex).__name__)
try:
    print('a0.sum()', "=>", a0.sum())
except Exception as ex:
    print('a0.sum()', "EXC", type(ex).__name__)
try:
    print('a3[::2]', "=>", a3[::2])
except Exception as ex:
    print('a3[::2]', "EXC", type(ex).__name__)
try:
    print('np.roll(a0, 1)', "=>", np.roll(a0, 1))
except Exception as ex:
    print('np.roll(a0, 1)', "EXC", type(ex).__name__)
try:
    print('np.floor(a0)', "=>", np.floor(a0))
except Exception as ex:
    print('np.floor(a0)', "EXC", type(ex).__name__)
try:
    print('a1.std(axis=0)', "=>", a1.std(axis=0))
except Exception as ex:
    print('a1.std(axis=0)', "EXC", type(ex).__name__)
try:
    print('a1.astype(int)', "=>", a1.astype(int))
except Exception as ex:
    print('a1.astype(int)', "EXC", type(ex).__name__)
try:
    print('np.ones_like(a3, dtype=float)', "=>", np.ones_like(a3, dtype=float))
except Exception as ex:
    print('np.ones_like(a3, dtype=float)', "EXC", type(ex).__name__)
try:
    print('np.zeros_like(a3)', "=>", np.zeros_like(a3))
except Exception as ex:
    print('np.zeros_like(a3)', "EXC", type(ex).__name__)
try:
    print('a1.std()', "=>", a1.std())
except Exception as ex:
    print('a1.std()', "EXC", type(ex).__name__)
try:
    print('a0.mean()', "=>", a0.mean())
except Exception as ex:
    print('a0.mean()', "EXC", type(ex).__name__)
try:
    print('a1 // 3', "=>", a1 // 3)
except Exception as ex:
    print('a1 // 3', "EXC", type(ex).__name__)
try:
    print('abs(a3)', "=>", abs(a3))
except Exception as ex:
    print('abs(a3)', "EXC", type(ex).__name__)
try:
    print('a0 / 3', "=>", a0 / 3)
except Exception as ex:
    print('a0 / 3', "EXC", type(ex).__name__)
try:
    print('abs(a2)', "=>", abs(a2))
except Exception as ex:
    print('abs(a2)', "EXC", type(ex).__name__)
try:
    print('a3.std(axis=0)', "=>", a3.std(axis=0))
except Exception as ex:
    print('a3.std(axis=0)', "EXC", type(ex).__name__)
try:
    print('a2 // a1', "=>", a2 // a1)
except Exception as ex:
    print('a2 // a1', "EXC", type(ex).__name__)
try:
    print('a0.argmax()', "=>", a0.argmax())
except Exception as ex:
    print('a0.argmax()', "EXC", type(ex).__name__)
try:
    print('a3.sum(axis=0)', "=>", a3.sum(axis=0))
except Exception as ex:
    print('a3.sum(axis=0)', "EXC", type(ex).__name__)
try:
    print('np.exp(a1 / 50)', "=>", np.exp(a1 / 50))
except Exception as ex:
    print('np.exp(a1 / 50)', "EXC", type(ex).__name__)
try:
    print('round(a2.ravel()[0], 2)', "=>", round(a2.ravel()[0], 2))
except Exception as ex:
    print('round(a2.ravel()[0], 2)', "EXC", type(ex).__name__)
try:
    print('np.where(a2 > 1, a2, -1)', "=>", np.where(a2 > 1, a2, -1))
except Exception as ex:
    print('np.where(a2 > 1, a2, -1)', "EXC", type(ex).__name__)
try:
    print('a3.T', "=>", a3.T)
except Exception as ex:
    print('a3.T', "EXC", type(ex).__name__)
try:
    print('np.tile(a2, 2)', "=>", np.tile(a2, 2))
except Exception as ex:
    print('np.tile(a2, 2)', "EXC", type(ex).__name__)
try:
    print('round(a3.ravel()[0], 2)', "=>", round(a3.ravel()[0], 2))
except Exception as ex:
    print('round(a3.ravel()[0], 2)', "EXC", type(ex).__name__)
try:
    print('a2.reshape(-1)', "=>", a2.reshape(-1))
except Exception as ex:
    print('a2.reshape(-1)', "EXC", type(ex).__name__)
try:
    print('a2.reshape(-1)', "=>", a2.reshape(-1))
except Exception as ex:
    print('a2.reshape(-1)', "EXC", type(ex).__name__)
try:
    print('a0 == a0', "=>", a0 == a0)
except Exception as ex:
    print('a0 == a0', "EXC", type(ex).__name__)
m = a0.copy()
try:
    m[0] = 7
    m[m > 3] = 0
    m += 1
    print(m)
except Exception as ex:
    print("EXC", type(ex).__name__)
