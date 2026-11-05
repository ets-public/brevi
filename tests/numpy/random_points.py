import numpy as np

# Fixed seed for reproducibility
rng = np.random.default_rng(42)
points = rng.random((10, 2))  # 10 random 2D points

# Pairwise differences via broadcasting: (N,1,2) - (1,N,2) -> (N,N,2)
diff = points[:, None, :] - points[None, :, :]
dist = np.sqrt((diff ** 2).sum(axis=-1))  # (N,N) distance matrix

# Ignore self-distances on the diagonal
np.fill_diagonal(dist, np.inf)

# Find index of minimum distance
i, j = np.unravel_index(np.argmin(dist), dist.shape)
i, j = sorted((i, j))

print("Points:")
print(points)
print(f"\nClosest pair: indices {i} and {j}")
print(f"Point {i}: {points[i]}")
print(f"Point {j}: {points[j]}")
print(f"Distance: {dist[i, j]:.6f}")
