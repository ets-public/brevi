// ---------- _brevi_net: sockets, TLS and HTTP/1.1 framing for Brevi's requests and httpx ----------
// The runtime itself reaches no network (caps test dynamic_runtime_reaches_nothing): every socket, TLS and
// environment call goes through NET, which only a program that imports Brevi's requests or httpx gets
// (src/dynpy/netglue.rs is pasted into it, and sets NET first thing in main). What is here is the part that
// needs no system access: a read buffer per connection, the response head and chunk-line framing, and
// gzip / zlib / raw deflate decoding (RFC 1950-1952), all over bytes the glue hands in.

pub struct NetErr {
    // the Python exception class: ConnectionRefusedError, TimeoutError, socket.gaierror, ssl.SSLError, ...
    pub kind: &'static str,
    pub msg: String,
}

pub struct NetOps {
    // blocking connect, with a timeout in seconds (< 0: none); a connection id
    pub connect: fn(&str, u16, f64) -> Result<i64, NetErr>,
    // a connect that does not wait; `ready` says when it is done (and raises its error)
    pub connect_nb: fn(&str, u16) -> Result<i64, NetErr>,
    pub ready: fn(i64) -> Result<bool, NetErr>,
    // TLS on the connection: server name, verify, CA file, CA dir, check hostname
    pub tls: fn(i64, &str, bool, &str, &str, bool) -> Result<(), NetErr>,
    // one handshake step: 0 done, 1 wants to read, 2 wants to write
    pub handshake: fn(i64) -> Result<i64, NetErr>,
    pub settimeout: fn(i64, f64),
    pub setblocking: fn(i64, bool),
    // bytes written, -1 / -2: would block (wants read / write)
    pub send: fn(i64, &[u8]) -> Result<i64, NetErr>,
    // Ok(bytes) (empty at the end), Err kind "want_read" / "want_write" when it would block
    pub recv: fn(i64, usize) -> Result<Vec<u8>, NetErr>,
    pub fileno: fn(i64) -> i64,
    pub close: fn(i64),
    // (fd, events) with 1 = read, 4 = write; revents per fd; timeout seconds (< 0: forever)
    pub poll: fn(&[(i64, i64)], f64) -> Result<Vec<i64>, NetErr>,
    // an environment variable (proxies, CA bundles), None under the sandbox
    pub env: fn(&str) -> Option<String>,
    // whether the sandbox lets the program reach this host
    pub host_ok: fn(&str) -> bool,
}

pub static NET: std::sync::OnceLock<NetOps> = std::sync::OnceLock::new();

thread_local! {
    static NET_BUF: RefCell<HashMap<i64, Vec<u8>>> = RefCell::new(HashMap::new());
    static NET_Z: RefCell<HashMap<i64, Inflater>> = RefCell::new(HashMap::new());
    static NET_ZN: Cell<i64> = Cell::new(0);
}

fn net() -> Result<&'static NetOps, E> {
    NET.get().ok_or_else(|| exc("OSError", "brevi: this program was built without network access".into()))
}

fn net_err(e: NetErr) -> E {
    exc(e.kind, e.msg)
}

fn net_id(v: &V) -> Result<i64, E> {
    match v {
        V::Int(i) => Ok(*i),
        _ => type_err("connection id must be int".into()),
    }
}

fn net_f(v: &V) -> f64 {
    match v {
        V::Int(i) => *i as f64,
        V::Float(f) => *f,
        _ => -1.0,
    }
}

fn net_s(v: &V) -> String {
    match v {
        V::Str(x) => x.s.clone(),
        _ => String::new(),
    }
}

fn find2(h: &[u8], n: &[u8]) -> Option<usize> {
    if h.len() < n.len() {
        return None;
    }
    (0..=h.len() - n.len()).find(|&i| &h[i..i + n.len()] == n)
}

pub fn brevi_net_fn(n: &str, a: Args) -> R {
    let arg = |i: usize| a.p.get(i).cloned().unwrap_or(V::None);
    match n {
        "open" => {
            let ops = net()?;
            let host = net_s(&arg(0));
            let id = (ops.connect)(&host, net_id(&arg(1))? as u16, net_f(&arg(2))).map_err(net_err)?;
            NET_BUF.with(|b| b.borrow_mut().insert(id, vec![]));
            Ok(V::Int(id))
        }
        "open_nb" => {
            let ops = net()?;
            let host = net_s(&arg(0));
            let id = (ops.connect_nb)(&host, net_id(&arg(1))? as u16).map_err(net_err)?;
            NET_BUF.with(|b| b.borrow_mut().insert(id, vec![]));
            Ok(V::Int(id))
        }
        "ready" => Ok(V::Bool((net()?.ready)(net_id(&arg(0))?).map_err(net_err)?)),
        "tls" => {
            let ops = net()?;
            (ops.tls)(net_id(&arg(0))?, &net_s(&arg(1)), truth(&arg(2))?, &net_s(&arg(3)), &net_s(&arg(4)), truth(&arg(5))?).map_err(net_err)?;
            Ok(V::None)
        }
        "handshake" => Ok(V::Int((net()?.handshake)(net_id(&arg(0))?).map_err(net_err)?)),
        "settimeout" => {
            (net()?.settimeout)(net_id(&arg(0))?, net_f(&arg(1)));
            Ok(V::None)
        }
        "setblocking" => {
            (net()?.setblocking)(net_id(&arg(0))?, truth(&arg(1))?);
            Ok(V::None)
        }
        // send(id, data): bytes written, or -1 / -2 when a non-blocking connection would block
        "send" => {
            let b = bytes_of(&arg(1)).ok_or_else(|| exc("TypeError", "a bytes-like object is required".into()))?;
            Ok(V::Int((net()?.send)(net_id(&arg(0))?, &b).map_err(net_err)?))
        }
        // sendall(id, data) on a blocking connection
        "sendall" => {
            let ops = net()?;
            let id = net_id(&arg(0))?;
            let b = bytes_of(&arg(1)).ok_or_else(|| exc("TypeError", "a bytes-like object is required".into()))?;
            let mut at = 0;
            while at < b.len() {
                let k = (ops.send)(id, &b[at..]).map_err(net_err)?;
                if k <= 0 {
                    return Err(exc("TimeoutError", "timed out".into()));
                }
                at += k as usize;
            }
            Ok(V::None)
        }
        // fill(id): reads what has arrived into the connection's buffer. Bytes read (0: the peer closed),
        // -1 / -2: a non-blocking connection would block (wants read / write)
        "fill" => {
            let ops = net()?;
            let id = net_id(&arg(0))?;
            match (ops.recv)(id, 65536) {
                Ok(b) => {
                    let k = b.len();
                    NET_BUF.with(|m| m.borrow_mut().entry(id).or_default().extend_from_slice(&b));
                    Ok(V::Int(k as i64))
                }
                Err(e) if e.kind == "want_read" => Ok(V::Int(-1)),
                Err(e) if e.kind == "want_write" => Ok(V::Int(-2)),
                Err(e) => Err(net_err(e)),
            }
        }
        // head(id): the response head (through the blank line) when it is all in the buffer, else None
        "head" => {
            let id = net_id(&arg(0))?;
            Ok(NET_BUF.with(|m| {
                let mut m = m.borrow_mut();
                let b = m.entry(id).or_default();
                // a blank line before the status line is skipped, as http.client and h11 do
                while b.starts_with(b"\r\n") {
                    b.drain(..2);
                }
                let end = match (find2(b, b"\r\n\r\n"), find2(b, b"\n\n")) {
                    (Some(x), Some(y)) if y + 2 < x + 4 => y + 2,
                    (Some(x), _) => x + 4,
                    (None, Some(y)) => y + 2,
                    (None, None) => return V::None,
                };
                bytesv(b.drain(..end).collect())
            }))
        }
        // line(id): one line (with its newline) when it is in the buffer, else None
        "line" => {
            let id = net_id(&arg(0))?;
            Ok(NET_BUF.with(|m| {
                let mut m = m.borrow_mut();
                let b = m.entry(id).or_default();
                match b.iter().position(|&c| c == b'\n') {
                    Some(k) => bytesv(b.drain(..=k).collect()),
                    None => V::None,
                }
            }))
        }
        // take(id, n): up to n buffered bytes (n < 0: all of them)
        "take" => {
            let id = net_id(&arg(0))?;
            let k = net_id(&arg(1))?;
            Ok(NET_BUF.with(|m| {
                let mut m = m.borrow_mut();
                let b = m.entry(id).or_default();
                let k = if k < 0 { b.len() } else { (k as usize).min(b.len()) };
                bytesv(b.drain(..k).collect())
            }))
        }
        "avail" => {
            let id = net_id(&arg(0))?;
            Ok(V::Int(NET_BUF.with(|m| m.borrow().get(&id).map_or(0, |b| b.len())) as i64))
        }
        "fileno" => Ok(V::Int((net()?.fileno)(net_id(&arg(0))?))),
        "close" => {
            let id = net_id(&arg(0))?;
            NET_BUF.with(|m| m.borrow_mut().remove(&id));
            if let Ok(ops) = net() {
                (ops.close)(id);
            }
            Ok(V::None)
        }
        // poll([(fd, events), ...], timeout): the events that happened, per fd
        "poll" => {
            let ops = net()?;
            let mut fds = vec![];
            for x in to_vec(&arg(0))? {
                let p = to_vec(&x)?;
                fds.push((net_id(&p[0])?, net_id(&p[1])?));
            }
            let r = (ops.poll)(&fds, net_f(&arg(1))).map_err(net_err)?;
            Ok(list(r.into_iter().map(V::Int).collect()))
        }
        "env" => Ok(match NET.get().and_then(|o| (o.env)(&net_s(&arg(0)))) {
            Some(v) => st(v),
            None => V::None,
        }),
        "host_ok" => Ok(V::Bool(NET.get().is_some_and(|o| (o.host_ok)(&net_s(&arg(0)))))),
        // zopen(mode): a decoder for "gzip", "zlib" or "deflate" (raw) data
        "zopen" => {
            let mode = match net_s(&arg(0)).as_str() {
                "gzip" => ZMode::Gzip,
                "zlib" => ZMode::Zlib,
                _ => ZMode::Raw,
            };
            let id = NET_ZN.with(|c| {
                c.set(c.get() + 1);
                c.get()
            });
            NET_Z.with(|z| z.borrow_mut().insert(id, Inflater::new(mode)));
            Ok(V::Int(id))
        }
        // zfeed(id, data): what the data decodes to so far (an error names zlib's message)
        "zfeed" => {
            let id = net_id(&arg(0))?;
            let b = bytes_of(&arg(1)).unwrap_or_default();
            NET_Z.with(|z| {
                let mut z = z.borrow_mut();
                let Some(f) = z.get_mut(&id) else { return Err(exc("ValueError", "decoder closed".into())) };
                f.feed(&b).map(bytesv).map_err(|m| exc("zlib.error", format!("Error -3 while decompressing data: {m}")))
            })
        }
        // zstate(id): 0 more data wanted, 1 the stream ended, 2 the stream ended and more data came after it
        "zstate" => {
            let id = net_id(&arg(0))?;
            Ok(V::Int(NET_Z.with(|z| z.borrow().get(&id).map_or(1, |f| f.state()))))
        }
        // zrest(id): the bytes after the end of the stream (a second gzip member)
        "zrest" => {
            let id = net_id(&arg(0))?;
            Ok(bytesv(NET_Z.with(|z| z.borrow().get(&id).map_or(vec![], |f| f.rest()))))
        }
        "zclose" => {
            let id = net_id(&arg(0))?;
            NET_Z.with(|z| z.borrow_mut().remove(&id));
            Ok(V::None)
        }
        _ => Err(unsupported(&format!("_brevi_net.{n}"))),
    }
}

// ---------- inflate ----------
// Decodes a block once all of it has arrived: a block cut short by the end of the data is decoded again from
// its start when more comes, so output arrives block by block, as zlib gives it (zlib may give it sooner).

#[derive(Clone, Copy, PartialEq)]
enum ZMode {
    Gzip,
    Zlib,
    Raw,
}

#[derive(Clone, Copy, PartialEq)]
enum ZStage {
    Header,
    Blocks,
    Trailer,
    Done,
}

struct Inflater {
    mode: ZMode,
    stage: ZStage,
    inp: Vec<u8>,
    // bit position in inp of what is not decoded yet
    bit: usize,
    // the last 32 KiB of output (the window) followed by output not handed out yet
    win: Vec<u8>,
    // where in win the output not handed out yet starts
    fresh: usize,
    crc: u32,
    adler: (u32, u32),
    size: u64,
}

struct ZMore;

enum ZE {
    More,
    Bad(String),
}

impl From<ZMore> for ZE {
    fn from(_: ZMore) -> ZE {
        ZE::More
    }
}

struct Huff {
    count: [u16; 16],
    sym: Vec<u16>,
}

impl Huff {
    fn new(lens: &[u8]) -> Result<Huff, String> {
        let mut count = [0u16; 16];
        for &l in lens {
            count[l as usize] += 1;
        }
        count[0] = 0;
        let mut left: i32 = 1;
        for l in 1..16 {
            left <<= 1;
            left -= count[l] as i32;
            if left < 0 {
                return Err("invalid code lengths set".into());
            }
        }
        let mut offs = [0u16; 16];
        for l in 1..15 {
            offs[l + 1] = offs[l] + count[l];
        }
        let mut sym = vec![0u16; lens.len()];
        for (s, &l) in lens.iter().enumerate() {
            if l != 0 {
                sym[offs[l as usize] as usize] = s as u16;
                offs[l as usize] += 1;
            }
        }
        Ok(Huff { count, sym })
    }
}

const Z_LBASE: [u16; 29] = [3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131, 163, 195, 227, 258];
const Z_LEXT: [u8; 29] = [0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0];
const Z_DBASE: [u16; 30] = [1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537, 2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577];
const Z_DEXT: [u8; 30] = [0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13, 13];

fn crc32_upd(mut c: u32, b: &[u8]) -> u32 {
    c = !c;
    for &x in b {
        c ^= x as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 { (c >> 1) ^ 0xEDB88320 } else { c >> 1 };
        }
    }
    !c
}

struct Bits<'a> {
    b: &'a [u8],
    pos: usize,
}

impl<'a> Bits<'a> {
    fn bit(&mut self) -> Result<u32, ZMore> {
        let byte = *self.b.get(self.pos >> 3).ok_or(ZMore)?;
        let v = (byte >> (self.pos & 7)) & 1;
        self.pos += 1;
        Ok(v as u32)
    }
    fn bits(&mut self, n: u32) -> Result<u32, ZMore> {
        let mut v = 0;
        for i in 0..n {
            v |= self.bit()? << i;
        }
        Ok(v)
    }
    fn decode(&mut self, h: &Huff) -> Result<u16, ZE> {
        let (mut code, mut first, mut index) = (0i32, 0i32, 0i32);
        for len in 1..16 {
            code |= self.bit()? as i32;
            let count = h.count[len] as i32;
            if code - count < first {
                return Ok(h.sym[(index + (code - first)) as usize]);
            }
            index += count;
            first += count;
            first <<= 1;
            code <<= 1;
        }
        Err(ZE::Bad("invalid code".into()))
    }
}

impl Inflater {
    fn new(mode: ZMode) -> Inflater {
        Inflater { mode, stage: if mode == ZMode::Raw { ZStage::Blocks } else { ZStage::Header }, inp: vec![], bit: 0, win: vec![], fresh: 0, crc: 0, adler: (1, 0), size: 0 }
    }

    fn state(&self) -> i64 {
        match self.stage {
            ZStage::Done if self.bit / 8 < self.inp.len() => 2,
            ZStage::Done => 1,
            _ => 0,
        }
    }

    fn rest(&self) -> Vec<u8> {
        self.inp[(self.bit / 8).min(self.inp.len())..].to_vec()
    }

    fn feed(&mut self, data: &[u8]) -> Result<Vec<u8>, String> {
        if self.stage == ZStage::Done {
            self.inp.extend_from_slice(data);
            return Ok(vec![]);
        }
        self.inp.extend_from_slice(data);
        loop {
            let r = match self.stage {
                ZStage::Header => self.header(),
                ZStage::Blocks => self.block(),
                ZStage::Trailer => self.trailer(),
                ZStage::Done => break,
            };
            match r {
                Ok(()) => {}
                Err(ZE::More) => break,
                Err(ZE::Bad(m)) => return Err(m),
            }
        }
        // hand out what is new, keep the window, drop input already decoded
        let out = self.win[self.fresh..].to_vec();
        if self.mode == ZMode::Gzip {
            self.crc = crc32_upd(self.crc, &out);
        } else if self.mode == ZMode::Zlib {
            let (mut a, mut b) = self.adler;
            for &x in &out {
                a = (a + x as u32) % 65521;
                b = (b + a) % 65521;
            }
            self.adler = (a, b);
        }
        self.size += out.len() as u64;
        if self.win.len() > 32768 {
            self.win.drain(..self.win.len() - 32768);
        }
        self.fresh = self.win.len();
        let used = self.bit / 8;
        if self.stage != ZStage::Done && used > 0 {
            self.inp.drain(..used);
            self.bit -= used * 8;
        }
        Ok(out)
    }

    fn header(&mut self) -> Result<(), ZE> {
        let b = &self.inp;
        let at = self.bit / 8;
        let need = |k: usize| if b.len() < at + k { Err(ZE::More) } else { Ok(()) };
        match self.mode {
            ZMode::Zlib => {
                need(2)?;
                let (cmf, flg) = (b[at], b[at + 1]);
                if ((cmf as u32) << 8 | flg as u32) % 31 != 0 {
                    return Err(ZE::Bad("incorrect header check".into()));
                }
                if cmf & 15 != 8 {
                    return Err(ZE::Bad("unknown compression method".into()));
                }
                if cmf >> 4 > 7 {
                    return Err(ZE::Bad("invalid window size".into()));
                }
                if flg & 0x20 != 0 {
                    return Err(ZE::Bad("need dictionary".into()));
                }
                self.bit = (at + 2) * 8;
            }
            _ => {
                need(10)?;
                if b[at] != 0x1f || b[at + 1] != 0x8b {
                    return Err(ZE::Bad("incorrect header check".into()));
                }
                if b[at + 2] != 8 {
                    return Err(ZE::Bad("unknown compression method".into()));
                }
                let flg = b[at + 3];
                let mut p = at + 10;
                if flg & 4 != 0 {
                    if b.len() < p + 2 {
                        return Err(ZE::More);
                    }
                    p += 2 + (b[p] as usize | (b[p + 1] as usize) << 8);
                }
                for f in [8u8, 16] {
                    if flg & f != 0 {
                        match b.get(p..).and_then(|s| s.iter().position(|&c| c == 0)) {
                            Some(k) => p += k + 1,
                            None => return Err(ZE::More),
                        }
                    }
                }
                if flg & 2 != 0 {
                    p += 2;
                }
                if b.len() < p {
                    return Err(ZE::More);
                }
                self.bit = p * 8;
                self.crc = 0;
                self.size = 0;
            }
        }
        self.stage = ZStage::Blocks;
        Ok(())
    }

    fn trailer(&mut self) -> Result<(), ZE> {
        let at = (self.bit + 7) / 8;
        // the trailer covers output already handed out: count what this call made too
        let pending = &self.win[self.fresh..];
        match self.mode {
            ZMode::Gzip => {
                if self.inp.len() < at + 8 {
                    return Err(ZE::More);
                }
                let t = &self.inp[at..at + 8];
                let crc = crc32_upd(self.crc, pending);
                let size = (self.size + pending.len() as u64) as u32;
                if u32::from_le_bytes([t[0], t[1], t[2], t[3]]) != crc {
                    return Err(ZE::Bad("incorrect data check".into()));
                }
                if u32::from_le_bytes([t[4], t[5], t[6], t[7]]) != size {
                    return Err(ZE::Bad("incorrect length check".into()));
                }
                self.bit = (at + 8) * 8;
            }
            ZMode::Zlib => {
                if self.inp.len() < at + 4 {
                    return Err(ZE::More);
                }
                let (mut a, mut b2) = self.adler;
                for &x in pending {
                    a = (a + x as u32) % 65521;
                    b2 = (b2 + a) % 65521;
                }
                let t = &self.inp[at..at + 4];
                if u32::from_be_bytes([t[0], t[1], t[2], t[3]]) != (b2 << 16 | a) {
                    return Err(ZE::Bad("incorrect data check".into()));
                }
                self.bit = (at + 4) * 8;
            }
            ZMode::Raw => self.bit = at * 8,
        }
        self.stage = ZStage::Done;
        Ok(())
    }

    // One whole block, or ZE::More with nothing changed.
    fn block(&mut self) -> Result<(), ZE> {
        let start_len = self.win.len();
        let mut bits = Bits { b: &self.inp, pos: self.bit };
        let r = Self::block_in(&mut bits, &mut self.win);
        match r {
            Ok(last) => {
                self.bit = bits.pos;
                if last {
                    self.stage = ZStage::Trailer;
                }
                Ok(())
            }
            Err(e) => {
                self.win.truncate(start_len);
                Err(e)
            }
        }
    }

    fn block_in(bits: &mut Bits, out: &mut Vec<u8>) -> Result<bool, ZE> {
        let last = bits.bit()? == 1;
        match bits.bits(2)? {
            0 => {
                bits.pos = (bits.pos + 7) & !7;
                let at = bits.pos / 8;
                if bits.b.len() < at + 4 {
                    return Err(ZE::More);
                }
                let len = bits.b[at] as usize | (bits.b[at + 1] as usize) << 8;
                let nlen = bits.b[at + 2] as usize | (bits.b[at + 3] as usize) << 8;
                if len != !nlen & 0xffff {
                    return Err(ZE::Bad("invalid stored block lengths".into()));
                }
                if bits.b.len() < at + 4 + len {
                    return Err(ZE::More);
                }
                out.extend_from_slice(&bits.b[at + 4..at + 4 + len]);
                bits.pos = (at + 4 + len) * 8;
            }
            1 => {
                let mut l = [0u8; 288];
                for (i, x) in l.iter_mut().enumerate() {
                    *x = if i < 144 { 8 } else if i < 256 { 9 } else if i < 280 { 7 } else { 8 };
                }
                let lit = Huff::new(&l).map_err(ZE::Bad)?;
                let dist = Huff::new(&[5u8; 30]).map_err(ZE::Bad)?;
                Self::codes(bits, out, &lit, &dist)?;
            }
            2 => {
                let nlen = bits.bits(5)? as usize + 257;
                let ndist = bits.bits(5)? as usize + 1;
                let ncode = bits.bits(4)? as usize + 4;
                if nlen > 286 || ndist > 30 {
                    return Err(ZE::Bad("too many length or distance symbols".into()));
                }
                const ORD: [usize; 19] = [16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15];
                let mut cl = [0u8; 19];
                for &o in ORD.iter().take(ncode) {
                    cl[o] = bits.bits(3)? as u8;
                }
                let lencode = Huff::new(&cl).map_err(|_| ZE::Bad("invalid code lengths set".into()))?;
                let mut lens = vec![0u8; nlen + ndist];
                let mut i = 0;
                while i < nlen + ndist {
                    let sym = bits.decode(&lencode)?;
                    if sym < 16 {
                        lens[i] = sym as u8;
                        i += 1;
                        continue;
                    }
                    let (val, rep) = match sym {
                        16 => {
                            if i == 0 {
                                return Err(ZE::Bad("invalid bit length repeat".into()));
                            }
                            (lens[i - 1], 3 + bits.bits(2)? as usize)
                        }
                        17 => (0, 3 + bits.bits(3)? as usize),
                        _ => (0, 11 + bits.bits(7)? as usize),
                    };
                    if i + rep > nlen + ndist {
                        return Err(ZE::Bad("invalid bit length repeat".into()));
                    }
                    for _ in 0..rep {
                        lens[i] = val;
                        i += 1;
                    }
                }
                if lens[256] == 0 {
                    return Err(ZE::Bad("invalid code -- missing end-of-block".into()));
                }
                let lit = Huff::new(&lens[..nlen]).map_err(|_| ZE::Bad("invalid literal/lengths set".into()))?;
                let dist = Huff::new(&lens[nlen..]).map_err(|_| ZE::Bad("invalid distances set".into()))?;
                Self::codes(bits, out, &lit, &dist)?;
            }
            _ => return Err(ZE::Bad("invalid block type".into())),
        }
        Ok(last)
    }

    fn codes(bits: &mut Bits, out: &mut Vec<u8>, lit: &Huff, dist: &Huff) -> Result<(), ZE> {
        loop {
            let sym = bits.decode(lit)? as usize;
            if sym < 256 {
                out.push(sym as u8);
            } else if sym == 256 {
                return Ok(());
            } else {
                let s = sym - 257;
                if s >= 29 {
                    return Err(ZE::Bad("invalid literal/length code".into()));
                }
                let len = Z_LBASE[s] as usize + bits.bits(Z_LEXT[s] as u32)? as usize;
                let ds = bits.decode(dist)? as usize;
                if ds >= 30 {
                    return Err(ZE::Bad("invalid distance code".into()));
                }
                let d = Z_DBASE[ds] as usize + bits.bits(Z_DEXT[ds] as u32)? as usize;
                if d > out.len() {
                    return Err(ZE::Bad("invalid distance too far back".into()));
                }
                let from = out.len() - d;
                for k in 0..len {
                    let c = out[from + k];
                    out.push(c);
                }
            }
        }
    }
}
