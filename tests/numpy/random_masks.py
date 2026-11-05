import numpy as np

rng = np.random.default_rng(42)
arr = rng.uniform(0, 100, size=20).round(2)
threshold = 50.0

# Boolean mask approach
mask = arr > threshold
mask_indices = np.flatnonzero(mask)

# np.where approach
where_indices = np.where(arr > threshold)[0]

print("Array:", arr)
print(f"Threshold: {threshold}")
print("Boolean mask:", mask)
print("Indices from boolean mask:", mask_indices)
print("Indices from np.where:", where_indices)
print("Values above threshold:", arr[mask])
print("Indices match:", np.array_equal(mask_indices, where_indices))
