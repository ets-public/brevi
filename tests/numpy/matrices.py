import numpy as np

# Fixed size for all matrices
n = 3

# Create the base matrices
identity = np.eye(n, dtype=int)
zeros = np.zeros((n, n), dtype=int)
ones = np.ones((n, n), dtype=int)

print("Identity matrix:")
print(identity)
print("\nZeros matrix:")
print(zeros)
print("\nOnes matrix:")
print(ones)

# Vertical stacking (rows added): result shape (3n, n)
v_stacked = np.vstack((identity, zeros, ones))
print("\nvstack (identity, zeros, ones) -> shape", v_stacked.shape)
print(v_stacked)

# Horizontal stacking (columns added): result shape (n, 3n)
h_stacked = np.hstack((identity, zeros, ones))
print("\nhstack (identity, zeros, ones) -> shape", h_stacked.shape)
print(h_stacked)

# Combine both: a 2x2 block matrix
block = np.vstack((
    np.hstack((identity, zeros)),
    np.hstack((ones, identity)),
))
print("\nBlock matrix [[I, 0], [1, I]] -> shape", block.shape)
print(block)
