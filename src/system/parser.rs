use super::lexer::{BuildFile, Span, Tokens};
use ariadne::{Color, Label, Report, ReportKind, Source};

#[derive(Debug, Clone)]
pub enum ShellPart {
    Literal(String),
    Interpolate(String),
}

#[derive(Debug, Clone)]
pub enum Expr {
    StringLit(String),
    Ident(String),
    Glob(String),
    Array(Vec<Expr>),
    Shell(Vec<ShellPart>),
}

#[derive(Debug, Clone)]
pub struct Rule {
    pub name: String,
    pub cmd: Vec<ShellPart>,
    pub depfile: Option<Vec<ShellPart>>,
}

#[derive(Debug, Clone)]
pub struct BuildEdge {
    pub inputs: Vec<Vec<ShellPart>>,
    pub outputs: Vec<Vec<ShellPart>>,
    pub rule: String,
    pub vars: Vec<(String, Expr)>,
}

#[derive(Debug, Clone)]
pub enum Stmt {
    Assign { name: String, value: Expr },
    Shell(Vec<ShellPart>),
    For { var: String, iter: Expr, body: Vec<Stmt> },
    Target { target: String, body: Vec<Stmt> },
    Run(String),
    Rule(Rule),
    Build(BuildEdge),
}

#[derive(PartialEq)]
enum StringContext { Array, Shell }

struct Parser<'a> {
    file: &'a BuildFile,
    tokens: Vec<(Tokens, Span)>,
    pos: usize,
}

impl<'a> Parser<'a> {
    fn new(file: &'a BuildFile, tokens: Vec<(Tokens, Span)>) -> Self {
        Self { file, tokens, pos: 0 }
    }

    fn peek(&self) -> &(Tokens, Span) {
        &self.tokens[self.pos.min(self.tokens.len() - 1)]
    }

    fn advance(&mut self) -> (Tokens, Span) {
        let tok = self.tokens[self.pos].clone();
        if self.pos + 1 < self.tokens.len() { self.pos += 1; }
        tok
    }

    fn at_end(&self) -> bool {
        matches!(self.peek().0, Tokens::EOF) || self.pos >= self.tokens.len()
    }

    fn check(&self, tok: &Tokens) -> bool {
        std::mem::discriminant(&self.peek().0) == std::mem::discriminant(tok)
    }

    fn expect(&mut self, expected: &Tokens, msg: &str) -> Result<(Tokens, Span), ()> {
        if self.check(expected) {
            Ok(self.advance())
        } else {
            let (_, span) = self.peek().clone();
            self.error(&span, msg);
            Err(())
        }
    }

    fn error(&self, span: &Span, message: &str) {
        let filename = self.file.name.as_str();
        Report::build(ReportKind::Error, (filename, span.start..span.end))
            .with_message(message)
            .with_label(
                Label::new((filename, span.start..span.end))
                    .with_message(message)
                    .with_color(Color::Red),
            )
            .finish()
            .print((filename, Source::from(self.file.src.as_str())))
            .unwrap();
    }

    fn skip_newlines(&mut self) {
        while self.check(&Tokens::Newline) { self.advance(); }
    }

    fn parse_shell_string(&mut self, ctx: StringContext) -> Result<Expr, ()> {
        self.expect(&Tokens::ShellBegin, "expected '\"'")?;
        let mut parts = Vec::new();
        while !self.check(&Tokens::ShellEnd) && !self.at_end() {
            let (tok, span) = self.advance();
            match tok {
                Tokens::Shell(s)       => parts.push(ShellPart::Literal(s)),
                Tokens::Interpolate(s) => parts.push(ShellPart::Interpolate(s)),
                _ => { self.error(&span, "unexpected token inside string"); return Err(()); }
            }
        }
        self.expect(&Tokens::ShellEnd, "expected closing '\"'")?;
        match ctx {
            StringContext::Array => {
                let s = parts.into_iter().map(|p| match p {
                    ShellPart::Literal(s)     => s,
                    ShellPart::Interpolate(s) => format!("$({s})"),
                }).collect();
                Ok(Expr::StringLit(s))
            }
            StringContext::Shell => Ok(Expr::Shell(parts)),
        }
    }

    fn parse_shell_parts(&mut self) -> Result<Vec<ShellPart>, ()> {
        match self.parse_shell_string(StringContext::Shell)? {
            Expr::Shell(parts) => Ok(parts),
            _ => unreachable!(),
        }
    }

    fn parse_path(&mut self) -> Result<Vec<ShellPart>, ()> {
        match &self.peek().0 {
            Tokens::ShellBegin => self.parse_shell_parts(),
            Tokens::Ident(_) => {
                let (Tokens::Ident(s), _) = self.advance() else { unreachable!() };
                Ok(vec![ShellPart::Literal(s)])
            }
            Tokens::Glob(_) => {
                let (Tokens::Glob(s), _) = self.advance() else { unreachable!() };
                Ok(vec![ShellPart::Literal(s)])
            }
            _ => {
                let (_, span) = self.peek().clone();
                self.error(&span, "expected file path");
                Err(())
            }
        }
    }

    fn parse_array(&mut self) -> Result<Expr, ()> {
        let mut elements = Vec::new();
        loop {
            self.skip_newlines();
            if self.check(&Tokens::ArrayEnd) || self.at_end() { break; }
            let expr = match &self.peek().0 {
                Tokens::ShellBegin   => self.parse_shell_string(StringContext::Array)?,
                Tokens::StringLit(_) => { let (Tokens::StringLit(s), _) = self.advance() else { unreachable!() }; Expr::StringLit(s) }
                Tokens::Ident(_)     => { let (Tokens::Ident(s), _)     = self.advance() else { unreachable!() }; Expr::Ident(s) }
                Tokens::Glob(_)      => { let (Tokens::Glob(s), _)      = self.advance() else { unreachable!() }; Expr::Glob(s) }
                _ => { let (_, span) = self.peek().clone(); self.error(&span, "expected value in array"); return Err(()); }
            };
            elements.push(expr);
            self.skip_newlines();
            if self.check(&Tokens::Comma) { self.advance(); }
        }
        self.expect(&Tokens::ArrayEnd, "expected ']'")?;
        Ok(Expr::Array(elements))
    }

    fn parse_expr(&mut self) -> Result<Expr, ()> {
        match &self.peek().0 {
            Tokens::ArrayBegin => { self.advance(); self.parse_array() }
            Tokens::Glob(_)    => { let (Tokens::Glob(s), _)  = self.advance() else { unreachable!() }; Ok(Expr::Glob(s)) }
            Tokens::Ident(_)   => { let (Tokens::Ident(s), _) = self.advance() else { unreachable!() }; Ok(Expr::Ident(s)) }
            Tokens::ShellBegin => self.parse_shell_string(StringContext::Shell),
            _ => { let (_, span) = self.peek().clone(); self.error(&span, "expected expression"); Err(()) }
        }
    }

    fn parse_stmt_block(&mut self) -> Result<Vec<Stmt>, ()> {
        self.expect(&Tokens::BraceOpen, "expected '{'")?;
        let mut stmts = Vec::new();
        loop {
            self.skip_newlines();
            if self.check(&Tokens::BraceClose) || self.at_end() { break; }
            match self.parse_stmt() {
                Ok(stmt) => stmts.push(stmt),
                Err(()) => {
                    while !self.at_end() && !self.check(&Tokens::Newline) && !self.check(&Tokens::BraceClose) {
                        self.advance();
                    }
                }
            }
        }
        self.expect(&Tokens::BraceClose, "expected '}'")?;
        Ok(stmts)
    }

    fn parse_ident_or_keyword(&mut self) -> Result<String, ()> {
        let (tok, span) = self.advance();
        match tok {
            Tokens::Ident(s) => Ok(s),
            Tokens::Rule     => Ok("rule".to_string()),
            Tokens::Build    => Ok("build".to_string()),
            Tokens::For      => Ok("for".to_string()),
            Tokens::In       => Ok("in".to_string()),
            Tokens::Target   => Ok("target".to_string()),
            Tokens::Run      => Ok("run".to_string()),
            _ => { self.error(&span, "expected identifier"); Err(()) }
        }
    }

    fn parse_rule(&mut self) -> Result<Stmt, ()> {
        let name = self.parse_ident_or_keyword()?;
        self.expect(&Tokens::BraceOpen, "expected '{' after rule name")?;
        let mut cmd = None;
        let mut depfile = None;
        loop {
            self.skip_newlines();
            if self.check(&Tokens::BraceClose) || self.at_end() { break; }
            let key = self.parse_ident_or_keyword()?;
            self.expect(&Tokens::Equals, "expected '='")?;
            let val = self.parse_shell_parts()?;
            match key.as_str() {
                "cmd"     => cmd = Some(val),
                "depfile" => depfile = Some(val),
                _ => { let (_, span) = self.peek().clone(); self.error(&span, "unknown rule field"); return Err(()); }
            }
            self.skip_newlines();
        }
        self.expect(&Tokens::BraceClose, "expected '}'")?;
        let cmd = cmd.ok_or_else(|| {
            let (_, span) = self.peek().clone();
            self.error(&span, "rule missing 'cmd' field");
        })?;
        Ok(Stmt::Rule(Rule { name, cmd, depfile }))
    }

    fn parse_build_edge(&mut self) -> Result<Stmt, ()> {
        let mut inputs = Vec::new();
        while !self.check(&Tokens::Arrow) && !self.at_end() {
            inputs.push(self.parse_path()?);
        }
        self.expect(&Tokens::Arrow, "expected '->'")?;
        let mut outputs = Vec::new();
        while !self.check(&Tokens::BraceOpen) && !self.check(&Tokens::Newline) && !self.at_end() {
            outputs.push(self.parse_path()?);
        }
        let mut vars = Vec::new();
        if self.check(&Tokens::BraceOpen) {
            self.advance();
            loop {
                self.skip_newlines();
                if self.check(&Tokens::BraceClose) || self.at_end() { break; }
                let key = self.parse_ident_or_keyword()?;
                self.expect(&Tokens::Equals, "expected '='")?;
                let val = self.parse_expr()?;
                vars.push((key, val));
                self.skip_newlines();
            }
            self.expect(&Tokens::BraceClose, "expected '}'")?;
        }
        let rule = vars.iter()
            .find(|(k, _)| k == "rule")
            .and_then(|(_, v)| if let Expr::Ident(s) = v { Some(s.clone()) } else { None })
            .ok_or_else(|| { let (_, span) = self.peek().clone(); self.error(&span, "build edge missing 'rule'"); })?;
        Ok(Stmt::Build(BuildEdge { inputs, outputs, rule, vars }))
    }

    fn parse_stmt(&mut self) -> Result<Stmt, ()> {
        match &self.peek().0 {
            Tokens::Rule  => { self.advance(); self.parse_rule() }
            Tokens::Build => { self.advance(); self.parse_build_edge() }
            Tokens::Target => {
                self.advance();
                let name = self.parse_ident_or_keyword()?;
                let body = self.parse_stmt_block()?;
                Ok(Stmt::Target { target: name, body })
            }
            Tokens::Run => {
                self.advance();
                let name = self.parse_ident_or_keyword()?;
                Ok(Stmt::Run(name))
            }
            Tokens::Ident(_) => {
                let name = self.parse_ident_or_keyword()?;
                self.expect(&Tokens::Equals, "expected '='")?;
                let value = self.parse_expr()?;
                Ok(Stmt::Assign { name, value })
            }
            Tokens::For => {
                self.advance();
                let var = self.parse_ident_or_keyword()?;
                self.expect(&Tokens::In, "expected 'in'")?;
                let iter = self.parse_expr()?;
                let body = self.parse_stmt_block()?;
                Ok(Stmt::For { var, iter, body })
            }
            Tokens::ShellBegin => {
                let parts = self.parse_shell_parts()?;
                Ok(Stmt::Shell(parts))
            }
            _ => {
                let (_, span) = self.peek().clone();
                self.error(&span, "expected statement");
                Err(())
            }
        }
    }

    fn parse_program(&mut self) -> Vec<Stmt> {
        let mut stmts = Vec::new();
        loop {
            self.skip_newlines();
            if self.at_end() { break; }
            match self.parse_stmt() {
                Ok(stmt) => stmts.push(stmt),
                Err(()) => {
                    while !self.at_end() && !self.check(&Tokens::Newline) {
                        self.advance();
                    }
                }
            }
        }
        stmts
    }
}

pub fn parse(file: &BuildFile, tokens: Vec<(Tokens, Span)>) -> Vec<Stmt> {
    let mut parser = Parser::new(file, tokens);
    parser.parse_program()
}