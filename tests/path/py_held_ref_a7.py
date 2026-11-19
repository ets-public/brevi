# A standard-library call that changes the list the typed build holds for append (heappop shrinks xs while
# xs.append waits for its argument): the dynamic build.
import heapq

xs = [5, 1, 4]
heapq.heapify(xs)
xs.append(heapq.heappop(xs))
print(xs)
