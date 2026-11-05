import numpy as np

# Fixed inputs: grades for 5 students across 3 assignments
grades = np.array([
    [88, 92, 79],
    [75, 85, 90],
    [95, 91, 84],
    [60, 70, 65],
    [82, 88, 94],
])

# Weights for each assignment (must sum to a positive value; np.average normalizes them)
weights = np.array([0.2, 0.3, 0.5])

# Weighted average per student (across assignments)
student_avgs = np.average(grades, axis=1, weights=weights)

# Weighted average per assignment across students, with student weights
student_weights = np.array([1, 2, 1, 1, 3])
assignment_avgs = np.average(grades, axis=0, weights=student_weights)

# Overall weighted average over all grades
overall_avg = np.average(grades, weights=np.outer(student_weights, weights))

print("Grades:\n", grades)
print("Assignment weights:", weights)
print("Student weights:", student_weights)
print()
print("Weighted average per student:")
for i, avg in enumerate(student_avgs, start=1):
    print(f"  Student {i}: {avg:.2f}")
print()
print("Weighted average per assignment:")
for j, avg in enumerate(assignment_avgs, start=1):
    print(f"  Assignment {j}: {avg:.2f}")
print()
print(f"Overall weighted average: {overall_avg:.2f}")
