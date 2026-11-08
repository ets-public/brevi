# np.save is not in Brevi's numpy (it writes a file): refused, never run by CPython's numpy.
import numpy as np

np.save("out.npy", np.zeros(3))
