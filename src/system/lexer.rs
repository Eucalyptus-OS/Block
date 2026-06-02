use std::{fs, path::PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Tokens {
    Target,
    Run,
    Ident(String),
    Equals,
    ArrayBegin,
    ArrayEnd,
    StringLit(String),
    Comma,
    Interpolate(String),
    Glob(String),
    BraceOpen,
    BraceClose,
    Arrow,
    Rule,
    Build,
    For,
    In,
    ShellBegin,
    Shell(String),
    ShellEnd,
    Newline,
    EOF,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    pub fn contains(&self, other: &Span) -> bool {
        self.start <= other.start && self.end >= other.end
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildFile {
    pub name: String,
    pub src: String,
    pub errors: Vec<(Span, String)>,
}

impl BuildFile {
    pub fn new(name: String, src: String) -> Self {
        Self { name, src, errors: vec![] }
    }
}

pub struct Lexer {
    file: BuildFile,
    chars: Vec<char>,
    pos: usize,
    line: usize,
}

impl Lexer {
    pub fn new(file: BuildFile) -> Self {
        let chars = file.src.chars().collect();
        Lexer { file, chars, pos: 0, line: 1 }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn peek_ahead(&self, offset: usize) -> Option<char> {
        self.chars.get(self.pos + offset).copied()
    }

    fn advance(&mut self) -> Option<char> {
        let c = self.chars.get(self.pos).copied();
        if c == Some('\n') { self.line += 1; }
        self.pos += 1;
        c
    }

    fn is_glob(s: &str) -> bool {
        s.contains('*') || s.contains('?')
    }

    fn read_word(&mut self) -> String {
        let mut word = String::new();
        while let Some(c) = self.peek() {
            if c.is_whitespace() || matches!(c, '=' | '{' | '}' | ',' | '[' | ']') {
                break;
            }
            word.push(c);
            self.advance();
        }
        word
    }

    pub fn tokenize(&mut self) -> Option<(Vec<(Tokens, Span)>, BuildFile)> {
        let mut tokens = Vec::new();

        while let Some(c) = self.peek() {
            let start = self.pos;
            match c {
                '\n' => {
                    self.advance();
                    tokens.push((Tokens::Newline, Span::new(start, self.pos)));
                }
                c if c.is_whitespace() => { self.advance(); continue; }
                '/' if self.peek_ahead(1) == Some('/') => {
                    while self.peek().is_some() && self.peek() != Some('\n') {
                        self.advance();
                    }
                    continue;
                }
                '-' if self.peek_ahead(1) == Some('>') => {
                    self.advance();
                    self.advance();
                    tokens.push((Tokens::Arrow, Span::new(start, self.pos)));
                }
                '[' => { self.advance(); tokens.push((Tokens::ArrayBegin, Span::new(start, self.pos))); }
                ']' => { self.advance(); tokens.push((Tokens::ArrayEnd,   Span::new(start, self.pos))); }
                '{' => { self.advance(); tokens.push((Tokens::BraceOpen,  Span::new(start, self.pos))); }
                '}' => { self.advance(); tokens.push((Tokens::BraceClose, Span::new(start, self.pos))); }
                ',' => { self.advance(); tokens.push((Tokens::Comma,      Span::new(start, self.pos))); }
                '=' => { self.advance(); tokens.push((Tokens::Equals,     Span::new(start, self.pos))); }
                '"' => {
                    self.advance();
                    tokens.push((Tokens::ShellBegin, Span::new(start, self.pos)));
                    let mut s = String::new();
                    while let Some(c) = self.peek() {
                        if c == '"' { break; }
                        if c == '$' && self.peek_ahead(1) == Some('(') {
                            if !s.is_empty() {
                                tokens.push((Tokens::Shell(s.drain(..).collect()), Span::new(start, self.pos)));
                            }
                            self.advance();
                            self.advance();
                            let interp_start = self.pos;
                            let mut name = String::new();
                            while let Some(c) = self.peek() {
                                if c == ')' { break; }
                                name.push(c);
                                self.advance();
                            }
                            self.advance();
                            tokens.push((Tokens::Interpolate(name), Span::new(interp_start, self.pos)));
                        } else {
                            s.push(c);
                            self.advance();
                        }
                    }
                    let shell_end = self.pos;
                    if !s.is_empty() {
                        tokens.push((Tokens::Shell(s), Span::new(start, shell_end)));
                    }
                    self.advance();
                    tokens.push((Tokens::ShellEnd, Span::new(shell_end, self.pos)));
                }
                '\'' => {
                    self.advance();
                    let lit_start = self.pos;
                    let mut s = String::new();
                    while let Some(c) = self.peek() {
                        if c == '\'' { break; }
                        s.push(c);
                        self.advance();
                    }
                    self.advance();
                    tokens.push((Tokens::StringLit(s), Span::new(lit_start, self.pos)));
                }
                _ => {
                    let word = self.read_word();
                    if word.is_empty() {
                        self.advance();
                        continue;
                    }
                    let span = Span::new(start, self.pos);
                    let tok = match word.as_str() {
                        "rule"   => Tokens::Rule,
                        "build"  => Tokens::Build,
                        "for"    => Tokens::For,
                        "in"     => Tokens::In,
                        "target" => Tokens::Target,
                        "run"    => Tokens::Run,
                        _ if Self::is_glob(&word) => Tokens::Glob(word),
                        _                          => Tokens::Ident(word),
                    };
                    tokens.push((tok, span));
                }
            }
        }

        tokens.push((Tokens::EOF, Span::new(self.pos, self.pos)));
        Some((tokens, self.file.clone()))
    }
}

pub fn lex(path: PathBuf) -> Option<(Vec<(Tokens, Span)>, BuildFile)> {
    let src = fs::read_to_string(&path).ok()?;
    let name = path.to_string_lossy().into_owned();
    let file = BuildFile::new(name, src);
    let mut lexer = Lexer::new(file);
    lexer.tokenize()
}