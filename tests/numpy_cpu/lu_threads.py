# numpy runs solve and inv from 100 x 100, and det from 20000 values, on OpenBLAS's threaded LU, whose order
# Brevi does not model: with more than one CPU Brevi refuses them, on one CPU it gives numpy's digits. The
# results here are exact, so they do not depend on that order.
import numpy as np

A = np.eye(100) * 2.0
print(np.linalg.solve(A, np.ones(100))[:3], np.linalg.inv(A)[0, :2])
print(np.linalg.det(np.eye(142) * 2.0), np.linalg.slogdet(np.eye(141) * 2.0)[1] / np.log(2.0))
