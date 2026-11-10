n=1025
print(n.to_bytes(2,"big"), int.from_bytes(b"\x04\x01","big"), (255).bit_length(), (5).bit_count())
# nearby cases checked against python3
print((1025).to_bytes(2,"little"), (255).to_bytes(1,"big"), (256).to_bytes(2,"big"))
print((0).to_bytes(0,"big"), (0).to_bytes(0,"big",signed=True), (1).to_bytes())
print((2**40).to_bytes(6,"big"), (2**64).to_bytes(9,"little"), (-2**100).to_bytes(13,"big",signed=True))
print((-1).to_bytes(2,"big",signed=True), (-128).to_bytes(1,"big",signed=True), (128).to_bytes(2,"big",signed=True))
print(int.from_bytes(b"\xff",signed=True), int.from_bytes(b"\x01\x00","little"), int.from_bytes(b""), int.from_bytes(bytes=b"ab"))
print(int.from_bytes(b"\x00\x00\x00\x00\x00\x00\x00\x00\x01","little"))
print((5).from_bytes(b"ab","big"), int.to_bytes(5,2,"big"))
print(True.from_bytes(b"\x01","big"), False.from_bytes(b"\x01","big"))
for f in [
    lambda: (256).to_bytes(1,"big"),
    lambda: (-1).to_bytes(2,"big"),
    lambda: (-129).to_bytes(1,"big",signed=True),
    lambda: (1).to_bytes(-1,"big"),
    lambda: (1).to_bytes(2,"middle"),
    lambda: (1).to_bytes(2,3),
    lambda: (1).to_bytes(2.0,"big"),
    lambda: (1).to_bytes(2**70,"big"),
    lambda: int.from_bytes(5),
    lambda: int.from_bytes(b"ab","middle"),
    lambda: int.from_bytes(),
    lambda: int.from_bytes(b"ab","big",True),
]:
    try:
        f()
    except Exception as e:
        print(type(e).__name__+":", e)
