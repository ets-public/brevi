# What an agent wrote first: count the primes below N and sum them (N on stdin).
import sys

def primes_below(n):
    sieve = [True] * n
    sieve[0:2] = [False, False]
    for i in range(2, int(n ** 0.5) + 1):
        if sieve[i]:
            for j in range(i * i, n, i):
                sieve[j] = False
    return [i for i, p in enumerate(sieve) if p]

def main():
    n = int(sys.stdin.read())
    ps = primes_below(n)
    total = 0
    for p in ps:
        total += p
    print(len(ps), "primes below", n)
    print("sum", totl)

main()
