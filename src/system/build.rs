use std::collections::HashMap;
use std::fs;
use std::process::Command;
use std::sync::atomic::Ordering;
use std::time::SystemTime;
use glob::glob;
use super::parser::{BuildEdge, Expr, Rule, ShellPart, Stmt};
use crate::DEBUG;

struct Builder {
    vars: HashMap<String, Vec<String>>,
    rules: HashMap<String, Rule>,
    targets: HashMap<String, Vec<Stmt>>,
}

impl Builder {
    fn new() -> Self {
        Self {
            vars: HashMap::new(),
            rules: HashMap::new(),
            targets: HashMap::new(),
        }
    }

    fn expand_parts(&self, parts: &[ShellPart]) -> String {
        parts.iter().map(|p| match p {
            ShellPart::Literal(s)        => s.clone(),
            ShellPart::Interpolate(name) => {
                self.vars.get(name)
                    .map(|v| v.join(" "))
                    .unwrap_or_else(|| format!("$({name})"))
            }
        }).collect()
    }

    fn expand_path(&self, parts: &[ShellPart]) -> String {
        self.expand_parts(parts)
    }

    fn expand_expr(&self, expr: &Expr) -> Vec<String> {
        match expr {
            Expr::StringLit(s)  => vec![s.clone()],
            Expr::Ident(name)   => self.vars.get(name).cloned().unwrap_or_else(|| vec![name.clone()]),
            Expr::Glob(pattern) => {
                let results: Vec<String> = glob(pattern).expect("invalid glob")
                    .filter_map(|e| e.ok())
                    .map(|p| p.to_string_lossy().into_owned())
                    .collect();
                if DEBUG.load(Ordering::Relaxed) {
                    eprintln!("glob {pattern:?} -> {results:?}");
                }
                results
            }
            Expr::Array(items)  => items.iter().flat_map(|i| self.expand_expr(i)).collect(),
            Expr::Shell(parts)  => vec![self.expand_parts(parts)],
        }
    }

    fn mtime(path: &str) -> Option<SystemTime> {
        fs::metadata(path).ok()?.modified().ok()
    }

    fn parse_dep_file(path: &str) -> Vec<String> {
        let Ok(contents) = fs::read_to_string(path) else { return vec![] };
        contents.split_once(':')
            .map(|(_, deps)| {
                deps.split_whitespace()
                    .filter(|s| !s.ends_with('\\'))
                    .map(|s| s.to_string())
                    .collect()
            })
            .unwrap_or_default()
    }

    fn needs_rebuild(outputs: &[String], depfile: &str) -> bool {
        let oldest = outputs.iter()
            .filter_map(|o| Self::mtime(o))
            .min();
        let Some(out_time) = oldest else { return true };
        let deps = Self::parse_dep_file(depfile);
        if deps.is_empty() { return true }
        deps.iter().any(|d| Self::mtime(d).map_or(true, |t| t > out_time))
    }

    fn exec_build_edge(&mut self, edge: &BuildEdge, dry_run: bool) {
        let Some(rule) = self.rules.get(&edge.rule).cloned() else {
            eprintln!("unknown rule: {}", edge.rule);
            return;
        };

        for (k, v) in &edge.vars {
            if k != "rule" {
                let expanded = self.expand_expr(v);
                self.vars.insert(k.clone(), expanded);
            }
        }

        let inputs: Vec<String>  = edge.inputs.iter().map(|p| self.expand_path(p)).collect();
        let outputs: Vec<String> = edge.outputs.iter().map(|p| self.expand_path(p)).collect();

        self.vars.insert("in".to_string(),  inputs);
        self.vars.insert("out".to_string(), outputs.clone());

        let depfile = rule.depfile.as_deref().map(|p| self.expand_parts(p));

        if let Some(ref df) = depfile {
            if !Self::needs_rebuild(&outputs, df) {
                if DEBUG.load(Ordering::Relaxed) {
                    println!("up to date: {}", outputs.join(" "));
                }
                self.cleanup_edge_vars(edge);
                return;
            }
        }

        for out in &outputs {
            if let Some(parent) = std::path::Path::new(out).parent() {
                if !parent.as_os_str().is_empty() {
                    fs::create_dir_all(parent).ok();
                }
            }
        }

        let cmd = self.expand_parts(&rule.cmd);
        if DEBUG.load(Ordering::Relaxed) || dry_run {
            println!("$ {cmd}");
        }
        if !dry_run {
            let status = Command::new("sh")
                .arg("-c")
                .arg(&cmd)
                .status()
                .expect("failed to run command");
            if !status.success() {
                eprintln!("command failed: {cmd}");
                std::process::exit(1);
            }
        }

        self.cleanup_edge_vars(edge);
    }

    fn cleanup_edge_vars(&mut self, edge: &BuildEdge) {
        for (k, _) in &edge.vars { self.vars.remove(k); }
        self.vars.remove("in");
        self.vars.remove("out");
    }

    fn run_shell(&self, parts: &[ShellPart], dry_run: bool) {
        let cmd = self.expand_parts(parts);
        if DEBUG.load(Ordering::Relaxed) || dry_run {
            println!("$ {cmd}");
        }
        if !dry_run {
            let status = Command::new("sh")
                .arg("-c")
                .arg(&cmd)
                .status()
                .expect("failed to run command");
            if !status.success() {
                eprintln!("command failed: {cmd}");
                std::process::exit(1);
            }
        }
    }

    fn run_target(&mut self, name: &str, dry_run: bool) {
        let Some(body) = self.targets.get(name).cloned() else {
            eprintln!("unknown target: {name}");
            return;
        };
        if DEBUG.load(Ordering::Relaxed) {
            eprintln!("running target: {name}");
        }
        // Execute sequentially — Rule stmts register themselves as they're hit,
        // so Build stmts that follow can find them.
        for stmt in body {
            self.exec_stmt(&stmt, dry_run);
        }
    }

    fn register_targets(targets: &mut HashMap<String, Vec<Stmt>>, stmts: &[Stmt]) {
        for stmt in stmts {
            if let Stmt::Target { target, body } = stmt {
                targets.insert(target.clone(), body.clone());
                // Recurse so nested target declarations are reachable via run
                Self::register_targets(targets, body);
            }
        }
    }

    fn exec_stmt(&mut self, stmt: &Stmt, dry_run: bool) {
        match stmt {
            Stmt::Assign { name, value } => {
                let expanded = self.expand_expr(value);
                if DEBUG.load(Ordering::Relaxed) {
                    eprintln!("{name} = {expanded:?}");
                }
                self.vars.insert(name.clone(), expanded);
            }
            Stmt::Shell(parts) => self.run_shell(parts, dry_run),
            Stmt::Rule(rule)   => { self.rules.insert(rule.name.clone(), rule.clone()); }
            Stmt::Build(edge)  => self.exec_build_edge(edge, dry_run),
            Stmt::For { var, iter, body } => {
                let values = self.expand_expr(iter);
                if DEBUG.load(Ordering::Relaxed) {
                    eprintln!("for {var} in {values:?}");
                }
                for val in values {
                    self.vars.insert(var.clone(), vec![val]);
                    for stmt in body {
                        self.exec_stmt(stmt, dry_run);
                    }
                }
                self.vars.remove(var);
            }
            Stmt::Target { target, body } => {
                // Lazy — body runs only when invoked via `run`.
                // Registration happened in the pre-pass.
                let _ = (target, body);
            }
            Stmt::Run(name) => {
                self.run_target(name, dry_run);
            }
        }
    }
}

pub fn build(statements: Vec<Stmt>, dry_run: bool, target: Option<&str>) {
    let mut builder = Builder::new();

    // Pre-pass: register top-level rules and all targets (recursively).
    for stmt in &statements {
        if let Stmt::Rule(rule) = stmt {
            builder.rules.insert(rule.name.clone(), rule.clone());
        }
    }
    Builder::register_targets(&mut builder.targets, &statements);

    match target {
        Some(name) => builder.run_target(name, dry_run),
        None => {
            for stmt in &statements {
                if !matches!(stmt, Stmt::Target { .. }) {
                    builder.exec_stmt(stmt, dry_run);
                }
            }
        }
    }
}