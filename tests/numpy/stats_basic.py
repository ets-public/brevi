import numpy as np

# Fixed exam scores for reproducibility
scores = np.array([72, 85, 91, 66, 78, 88, 95, 59, 81, 74, 69, 90, 83, 77, 62])

mean = np.mean(scores)
median = np.median(scores)
std = np.std(scores, ddof=1)  # sample standard deviation
percentiles = np.percentile(scores, [10, 25, 50, 75, 90])

print(f"Scores: {scores.tolist()}")
print(f"Mean:   {mean:.2f}")
print(f"Median: {median:.2f}")
print(f"Std:    {std:.2f}")
for p, v in zip([10, 25, 50, 75, 90], percentiles):
    print(f"{p}th percentile: {v:.2f}")
