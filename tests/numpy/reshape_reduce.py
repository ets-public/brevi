import numpy as np

# Fixed input: 1-D array of 12 numbers
data = np.arange(1, 13)

# Reshape into 3x4 and 4x3
a = data.reshape(3, 4)
b = data.reshape(4, 3)

print("3x4 array:")
print(a)
print("Sum along axis 0 (columns):", a.sum(axis=0))
print("Sum along axis 1 (rows):", a.sum(axis=1))

print("\n4x3 array:")
print(b)
print("Sum along axis 0 (columns):", b.sum(axis=0))
print("Sum along axis 1 (rows):", b.sum(axis=1))
