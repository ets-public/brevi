import random
random.seed(7)
W=[''.join(random.choice('abcdefghij') for _ in range(random.randint(2,6))) for _ in range(5000)]
open('words.in','w').write('\n'.join(' '.join(random.choice(W) for _ in range(20)) for _ in range(100000))+'\n')
n=100000
L=[f"{n} {n*5}"]
for _ in range(n*5):
    L.append(f"{random.randrange(n)} {random.randrange(n)} {random.randint(1,100)}")
open('graph.in','w').write('\n'.join(L)+'\n')
open('sieve.in','w').write("5000000\n")
D=[f"d{i}" for i in range(200)]
open('records.in','w').write('\n'.join(f"{random.choice(W)},{random.choice(D)},{random.randint(1,10000)}" for _ in range(1000000))+'\n')
