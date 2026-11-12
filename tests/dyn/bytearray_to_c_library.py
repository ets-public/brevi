# A bytearray (or a bytearray subclass with no methods) handed to the C code of array, struct and zlib, which
# CPython runs: the library sees its bytes and its writes reach the program's bytearray.
import array, struct, zlib
x = array.array('B')
x.frombytes(bytearray(b'abc'))
print(x)
print(array.array('B', bytearray(b'xy')))
x.extend(bytearray(b'z'))
print(x)
ba = bytearray(8)
view = ba
struct.pack_into('<ii', ba, 0, 7, -1)
print(ba, view is ba)
print(struct.unpack('<ii', ba), struct.unpack_from('<i', ba, 4))
s = struct.Struct('<h')
print(s.unpack(bytearray(b'\x02\x01')))
s.pack_into(ba, 2, 513)
print(ba)
print(zlib.crc32(bytearray(b'hello')), zlib.decompress(zlib.compress(bytearray(b'hi'))))
try:
    struct.pack_into('<i', ba, 6, 1)
except struct.error as e:
    print('error:', e, ba)

class CB(bytearray):
    pass
print(array.array('B', CB(b'xy')))
c = CB(4)
struct.pack_into('<i', c, 0, 5)
print(c, type(c).__name__)
x.frombytes(CB(b'q'))
print(x)
