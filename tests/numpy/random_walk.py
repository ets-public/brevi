import numpy as np

# Fixed parameters
SEED = 42
N_STEPS = 10_000

# Reproducible random number generator
rng = np.random.default_rng(SEED)

# Each step is -1 or +1 with equal probability
steps = rng.choice([-1, 1], size=N_STEPS)

# Position over time (starting at 0)
positions = np.concatenate(([0], np.cumsum(steps)))

# Results
final_position = positions[-1]
max_excursion = np.max(np.abs(positions))
max_idx = int(np.argmax(np.abs(positions)))

print(f"Seed:             {SEED}")
print(f"Steps:            {N_STEPS}")
print(f"Final position:   {final_position}")
print(f"Max excursion:    {max_excursion} (at step {max_idx}, position {positions[max_idx]})")
print(f"Max position:     {positions.max()}")
print(f"Min position:     {positions.min()}")
