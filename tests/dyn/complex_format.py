def t(v, s):
    try:
        print(repr(format(v, s)))
    except Exception as e:
        print('raises', type(e).__name__, str(e))
vals = [1+2j, 1j, -1j, 0j, complex(-0.0, 0), 1234567.891-2.5e-7j, complex(float('inf'), float('nan')), complex(1e20, -1e-20), 3.25-4j, complex(0, -0.0)]
specs = ['', '.2f', 'f', 'e', 'E', 'g', 'G', 'n', '.3', '.0f', '+', '-', ' ', '+.2f', ' .1e', '>20', '<20', '^21', '*^21.3f', '20', '020', '0>20', '=20', ',', '_', ',.2f', '_g',
         'z.1f', '#', '#.2f', '#g', 'd', 's', '%', 'x', '.f', '.2ff', ',n', '10.3g', '-^15g', 'z', '+z.0f', '.17g', '.30f', '30.20e']
for v in vals:
    for s in specs:
        t(v, s)
c = 1.5-2j
print(f"{c!r:>12}|{c:^14.1f}|{c:+}|{c!s}")
print('{:.3e}'.format(c), '{0.real}{0.imag}'.format(c))
