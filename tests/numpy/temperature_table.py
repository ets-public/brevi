import numpy as np

# Fixed inputs: temperature range in Celsius
start_c = -40.0
stop_c = 100.0
num_points = 8

# Generate evenly spaced Celsius temperatures
celsius = np.linspace(start_c, stop_c, num_points)

# Convert to Fahrenheit and Kelvin
fahrenheit = celsius * 9 / 5 + 32
kelvin = celsius + 273.15

# Print the conversion table
header = f"{'Celsius':>10} {'Fahrenheit':>12} {'Kelvin':>10}"
print(header)
print("-" * len(header))
for c, f, k in zip(celsius, fahrenheit, kelvin):
    print(f"{c:10.2f} {f:12.2f} {k:10.2f}")

# Sanity check: -40 C should equal -40 F
assert np.isclose(fahrenheit[0], -40.0)
print("\nCheck passed: -40°C == -40°F")
