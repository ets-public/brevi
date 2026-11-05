import numpy as np
a0 = np.array([[[55.2, 80.0, 73.689, 15.0, -18.4], [53.1, -85.6137, -75.0, -98.937373, 96.484222], [-37.3, 35.6, 93.7, 78.75, 1e9]], [[-71.0, 63.4801, 0.1, 0.1, -3.85], [-5.5, 0.0, -28.471, 1e9, 16.0], [41.5517, -60.695666, 63.899, -29.0414, -7.43]], [[59.445336, -13.0, 2.58, -32.00809, -37.51], [59.75472, 37.835, -90.2896, -18.8, -31.23], [-16.0, 27.664875, 67.7, 12.8, 58.0]]])
a1 = np.array([68.95, 84.4007, 95.202, 37.77])
a2 = np.array([[5, -11, -13], [4, 14, -9], [20, 11, 1]])
a3 = np.array([-3])
np.set_printoptions(precision=8, suppress=False)
try:
    print('a2 == a0', "=>", a2 == a0)
except Exception as ex:
    print('a2 == a0', "EXC", type(ex).__name__)
try:
    print('a2 + a0', "=>", a2 + a0)
except Exception as ex:
    print('a2 + a0', "EXC", type(ex).__name__)
try:
    print('a2[np.newaxis]', "=>", a2[np.newaxis])
except Exception as ex:
    print('a2[np.newaxis]', "EXC", type(ex).__name__)
try:
    print('a2 == a2', "=>", a2 == a2)
except Exception as ex:
    print('a2 == a2', "EXC", type(ex).__name__)
try:
    print('np.nansum(a2)', "=>", np.nansum(a2))
except Exception as ex:
    print('np.nansum(a2)', "EXC", type(ex).__name__)
try:
    print('a1.max(axis=-1)', "=>", a1.max(axis=-1))
except Exception as ex:
    print('a1.max(axis=-1)', "EXC", type(ex).__name__)
try:
    print('a2 / 3', "=>", a2 / 3)
except Exception as ex:
    print('a2 / 3', "EXC", type(ex).__name__)
try:
    print('a2 % 4', "=>", a2 % 4)
except Exception as ex:
    print('a2 % 4', "EXC", type(ex).__name__)
try:
    print('a2 * 2.5', "=>", a2 * 2.5)
except Exception as ex:
    print('a2 * 2.5', "EXC", type(ex).__name__)
try:
    print('max(a2.ravel())', "=>", max(a2.ravel()))
except Exception as ex:
    print('max(a2.ravel())', "EXC", type(ex).__name__)
try:
    print('float(a1.ravel()[0])', "=>", float(a1.ravel()[0]))
except Exception as ex:
    print('float(a1.ravel()[0])', "EXC", type(ex).__name__)
try:
    print('a2.astype(bool)', "=>", a2.astype(bool))
except Exception as ex:
    print('a2.astype(bool)', "EXC", type(ex).__name__)
try:
    print('{"v": a1.ravel()[0]}', "=>", {"v": a1.ravel()[0]})
except Exception as ex:
    print('{"v": a1.ravel()[0]}', "EXC", type(ex).__name__)
try:
    print('np.concatenate([a3, a3])', "=>", np.concatenate([a3, a3]))
except Exception as ex:
    print('np.concatenate([a3, a3])', "EXC", type(ex).__name__)
try:
    print('a0[1:]', "=>", a0[1:])
except Exception as ex:
    print('a0[1:]', "EXC", type(ex).__name__)
try:
    print('np.mean(a3)', "=>", np.mean(a3))
except Exception as ex:
    print('np.mean(a3)', "EXC", type(ex).__name__)
try:
    print('np.unique(a2)', "=>", np.unique(a2))
except Exception as ex:
    print('np.unique(a2)', "EXC", type(ex).__name__)
try:
    print('np.mean(a1)', "=>", np.mean(a1))
except Exception as ex:
    print('np.mean(a1)', "EXC", type(ex).__name__)
try:
    print('a1 + a1', "=>", a1 + a1)
except Exception as ex:
    print('a1 + a1', "EXC", type(ex).__name__)
try:
    print('a1.mean(axis=0)', "=>", a1.mean(axis=0))
except Exception as ex:
    print('a1.mean(axis=0)', "EXC", type(ex).__name__)
try:
    print('a1.min()', "=>", a1.min())
except Exception as ex:
    print('a1.min()', "EXC", type(ex).__name__)
try:
    print('np.vstack([a1, a1])', "=>", np.vstack([a1, a1]))
except Exception as ex:
    print('np.vstack([a1, a1])', "EXC", type(ex).__name__)
try:
    print('np.argsort(a1, kind="stable")', "=>", np.argsort(a1, kind="stable"))
except Exception as ex:
    print('np.argsort(a1, kind="stable")', "EXC", type(ex).__name__)
try:
    print('a0.sum(axis=-1)', "=>", a0.sum(axis=-1))
except Exception as ex:
    print('a0.sum(axis=-1)', "EXC", type(ex).__name__)
try:
    print('a2 // 3', "=>", a2 // 3)
except Exception as ex:
    print('a2 // 3', "EXC", type(ex).__name__)
try:
    print('np.flip(a0)', "=>", np.flip(a0))
except Exception as ex:
    print('np.flip(a0)', "EXC", type(ex).__name__)
try:
    print('max(a2.ravel())', "=>", max(a2.ravel()))
except Exception as ex:
    print('max(a2.ravel())', "EXC", type(ex).__name__)
try:
    print('np.square(a3)', "=>", np.square(a3))
except Exception as ex:
    print('np.square(a3)', "EXC", type(ex).__name__)
try:
    print('a0.sum()', "=>", a0.sum())
except Exception as ex:
    print('a0.sum()', "EXC", type(ex).__name__)
try:
    print('a2 ** 2', "=>", a2 ** 2)
except Exception as ex:
    print('a2 ** 2', "EXC", type(ex).__name__)
try:
    print('sum(a2.ravel())', "=>", sum(a2.ravel()))
except Exception as ex:
    print('sum(a2.ravel())', "EXC", type(ex).__name__)
try:
    print('np.square(a1)', "=>", np.square(a1))
except Exception as ex:
    print('np.square(a1)', "EXC", type(ex).__name__)
try:
    print('a2.astype(int)', "=>", a2.astype(int))
except Exception as ex:
    print('a2.astype(int)', "EXC", type(ex).__name__)
try:
    print('np.sort(a1)', "=>", np.sort(a1))
except Exception as ex:
    print('np.sort(a1)', "EXC", type(ex).__name__)
try:
    print('a3.shape', "=>", a3.shape)
except Exception as ex:
    print('a3.shape', "EXC", type(ex).__name__)
try:
    print('a0 & a2', "=>", a0 & a2)
except Exception as ex:
    print('a0 & a2', "EXC", type(ex).__name__)
try:
    print('a0.sum(axis=0, keepdims=True)', "=>", a0.sum(axis=0, keepdims=True))
except Exception as ex:
    print('a0.sum(axis=0, keepdims=True)', "EXC", type(ex).__name__)
try:
    print('np.stack([a2, a2], axis=1)', "=>", np.stack([a2, a2], axis=1))
except Exception as ex:
    print('np.stack([a2, a2], axis=1)', "EXC", type(ex).__name__)
try:
    print('a3.min()', "=>", a3.min())
except Exception as ex:
    print('a3.min()', "EXC", type(ex).__name__)
try:
    print('a1 / 3', "=>", a1 / 3)
except Exception as ex:
    print('a1 / 3', "EXC", type(ex).__name__)
try:
    print('np.exp(a3 / 50)', "=>", np.exp(a3 / 50))
except Exception as ex:
    print('np.exp(a3 / 50)', "EXC", type(ex).__name__)
try:
    print('{"v": a1.ravel()[0]}', "=>", {"v": a1.ravel()[0]})
except Exception as ex:
    print('{"v": a1.ravel()[0]}', "EXC", type(ex).__name__)
try:
    print('list(a2)', "=>", list(a2))
except Exception as ex:
    print('list(a2)', "EXC", type(ex).__name__)
try:
    print('np.vstack([a3, a3])', "=>", np.vstack([a3, a3]))
except Exception as ex:
    print('np.vstack([a3, a3])', "EXC", type(ex).__name__)
try:
    print('a2 % 4', "=>", a2 % 4)
except Exception as ex:
    print('a2 % 4', "EXC", type(ex).__name__)
try:
    print('np.round(a1, 1)', "=>", np.round(a1, 1))
except Exception as ex:
    print('np.round(a1, 1)', "EXC", type(ex).__name__)
try:
    print('np.mean(a0)', "=>", np.mean(a0))
except Exception as ex:
    print('np.mean(a0)', "EXC", type(ex).__name__)
try:
    print('a2 > a0', "=>", a2 > a0)
except Exception as ex:
    print('a2 > a0', "EXC", type(ex).__name__)
try:
    print('-a1', "=>", -a1)
except Exception as ex:
    print('-a1', "EXC", type(ex).__name__)
try:
    print('np.mean(a3)', "=>", np.mean(a3))
except Exception as ex:
    print('np.mean(a3)', "EXC", type(ex).__name__)
try:
    print('np.mean(a3)', "=>", np.mean(a3))
except Exception as ex:
    print('np.mean(a3)', "EXC", type(ex).__name__)
try:
    print('a3[0]', "=>", a3[0])
except Exception as ex:
    print('a3[0]', "EXC", type(ex).__name__)
try:
    print('a2.T', "=>", a2.T)
except Exception as ex:
    print('a2.T', "EXC", type(ex).__name__)
try:
    print('abs(a3)', "=>", abs(a3))
except Exception as ex:
    print('abs(a3)', "EXC", type(ex).__name__)
try:
    print('a1 // 3', "=>", a1 // 3)
except Exception as ex:
    print('a1 // 3', "EXC", type(ex).__name__)
try:
    print('sum(a2.ravel())', "=>", sum(a2.ravel()))
except Exception as ex:
    print('sum(a2.ravel())', "EXC", type(ex).__name__)
try:
    print('max(a0.ravel())', "=>", max(a0.ravel()))
except Exception as ex:
    print('max(a0.ravel())', "EXC", type(ex).__name__)
try:
    print('a2.sum()', "=>", a2.sum())
except Exception as ex:
    print('a2.sum()', "EXC", type(ex).__name__)
try:
    print('a0[..., 0]', "=>", a0[..., 0])
except Exception as ex:
    print('a0[..., 0]', "EXC", type(ex).__name__)
try:
    print('np.zeros_like(a3)', "=>", np.zeros_like(a3))
except Exception as ex:
    print('np.zeros_like(a3)', "EXC", type(ex).__name__)
m = a0.copy()
try:
    m[0] = 7
    m[m > 3] = 0
    m += 1
    print(m)
except Exception as ex:
    print("EXC", type(ex).__name__)
