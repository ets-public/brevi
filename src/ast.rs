#[derive(Debug, Clone)]
pub enum Ty {
    Named(String),         // int, f64, bool, str, or a user type
    List(Box<Ty>),         // [T]
    Map(Box<Ty>, Box<Ty>), // {K: V}
    Tuple(Vec<Ty>),        // (A, B)
    Unit,
}

#[derive(Debug, Clone)]
pub struct Param {
    pub name: String,
    pub ty: Ty,
    pub is_mut: bool,
}

#[derive(Debug, Clone)]
pub struct Field {
    pub name: String,
    pub ty: Ty,
    pub default: Option<Expr>, // `cancelled: bool = false` lets constructors leave it out
}

#[derive(Debug, Clone)]
pub struct Variant {
    pub name: String,
    pub fields: Vec<Field>,
}

#[derive(Debug, Clone)]
pub enum Item {
    Fn {
        name: String,
        params: Vec<Param>,
        ret: Ty,
        fallible: bool,
        body: Expr,
        line: usize,
        script: bool, // made from top-level statements
    },
    Type {
        name: String,
        fields: Vec<Field>,
        line: usize,
    },
    Enum {
        name: String,
        variants: Vec<Variant>,
        line: usize,
    },
    Test {
        name: String,
        body: Expr,
        line: usize,
    },
}

#[derive(Debug, Clone)]
pub enum Pat {
    Wild,
    Bind(String),
    Int(i64),
    Str(String),
    Bool(bool),
    Tuple(Vec<Pat>),
    Variant(String, Vec<Pat>),
}

#[derive(Debug, Clone)]
pub enum Stmt {
    Let {
        name: Pat,
        ty: Option<Ty>,
        value: Expr,
        mutable: bool,
        line: usize,
    },
    Assign {
        target: Expr,
        op: &'static str,
        value: Expr,
        line: usize,
    },
    Expr {
        expr: Expr,
        line: usize,
    },
    Return {
        value: Option<Expr>,
        line: usize,
    },
    Break {
        line: usize,
    },
    Continue {
        line: usize,
    },
    Assert {
        cond: Expr,
        line: usize,
    },
}

#[derive(Debug, Clone)]
pub enum StrPart {
    Lit(String),
    Interp(Expr),
}

#[derive(Debug, Clone)]
pub enum Expr {
    Int(i64),
    Float(f64),
    Bool(bool),
    Str(Vec<StrPart>),
    Ident(String),
    Move(String), // the variable's last read: its value moves instead of being copied
    List(Vec<Expr>),
    Map(Vec<(Expr, Expr)>),
    Tuple(Vec<Expr>),
    Unary(&'static str, Box<Expr>),
    Binary(&'static str, Box<Expr>, Box<Expr>),
    // Calls keep argument names so constructors like Point(x: 1.0, y: 2.0) work.
    Call(String, Vec<(Option<String>, Expr)>, usize),
    Method(Box<Expr>, String, Vec<Expr>, usize),
    Field(Box<Expr>, String),
    TupleField(Box<Expr>, usize),
    Index(Box<Expr>, Box<Expr>),
    Lambda(Vec<String>, Box<Expr>),
    Try(Box<Expr>),
    If(Box<Expr>, Box<Expr>, Option<Box<Expr>>),
    Match(Box<Expr>, Vec<(Pat, Expr)>),
    For(Pat, Box<Expr>, Box<Expr>),
    While(Box<Expr>, Box<Expr>),
    Range(Box<Expr>, Box<Expr>),
    Block(Vec<Stmt>, Option<Box<Expr>>),
}
