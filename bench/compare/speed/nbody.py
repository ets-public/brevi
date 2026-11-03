import math

class Body:
    def __init__(self, x, y, z, vx, vy, vz, m):
        self.x = x; self.y = y; self.z = z
        self.vx = vx; self.vy = vy; self.vz = vz
        self.m = m

def energy(bodies):
    e = 0.0
    for i in range(len(bodies)):
        a = bodies[i]
        e += 0.5 * a.m * (a.vx * a.vx + a.vy * a.vy + a.vz * a.vz)
        for j in range(i + 1, len(bodies)):
            b = bodies[j]
            dx = a.x - b.x; dy = a.y - b.y; dz = a.z - b.z
            e -= a.m * b.m / math.sqrt(dx * dx + dy * dy + dz * dz)
    return e

def advance(bodies, dt):
    n = len(bodies)
    for i in range(n):
        a = bodies[i]
        for j in range(i + 1, n):
            b = bodies[j]
            dx = a.x - b.x; dy = a.y - b.y; dz = a.z - b.z
            d2 = dx * dx + dy * dy + dz * dz
            mag = dt / (d2 * math.sqrt(d2))
            a.vx -= dx * b.m * mag; a.vy -= dy * b.m * mag; a.vz -= dz * b.m * mag
            b.vx += dx * a.m * mag; b.vy += dy * a.m * mag; b.vz += dz * a.m * mag
    for b in bodies:
        b.x += dt * b.vx; b.y += dt * b.vy; b.z += dt * b.vz

def main():
    steps = int(input())
    pi = 3.141592653589793
    sm = 4 * pi * pi
    dp = 365.24
    bodies = [
        Body(0.0, 0.0, 0.0, 0.0, 0.0, 0.0, sm),
        Body(4.84143144246472090e+00, -1.16032004402742839e+00, -1.03622044471123109e-01,
             1.66007664274403694e-03 * dp, 7.69901118419740425e-03 * dp, -6.90460016972063023e-05 * dp, 9.54791938424326609e-04 * sm),
        Body(8.34336671824457987e+00, 4.12479856412430479e+00, -4.03523417114321381e-01,
             -2.76742510726862411e-03 * dp, 4.99852801234917238e-03 * dp, 2.30417297573763929e-05 * dp, 2.85885980666130812e-04 * sm),
        Body(1.28943695621391310e+01, -1.51111514016986312e+01, -2.23307578892655734e-01,
             2.96460137564761618e-03 * dp, 2.37847173959480950e-03 * dp, -2.96589568540237556e-05 * dp, 4.36624404335156298e-05 * sm),
        Body(1.53796971148509165e+01, -2.59193146099879641e+01, 1.79258772950371181e-01,
             2.68067772490389322e-03 * dp, 1.62824170038242295e-03 * dp, -9.51592254519715870e-05 * dp, 5.15138902046611451e-05 * sm),
    ]
    px = py = pz = 0.0
    for b in bodies:
        px += b.vx * b.m; py += b.vy * b.m; pz += b.vz * b.m
    bodies[0].vx = -px / sm; bodies[0].vy = -py / sm; bodies[0].vz = -pz / sm
    print(f"{energy(bodies):.9f}")
    for _ in range(steps):
        advance(bodies, 0.01)
    print(f"{energy(bodies):.9f}")

main()
