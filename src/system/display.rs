use super::parser::{BuildEdge, Expr, Rule, ShellPart, Stmt};

const RST:  &str = "\x1b[0m";
const BOLD: &str = "\x1b[1m";
const DIM:  &str = "\x1b[2m";

const KW:     &str = "\x1b[38;5;213m"; // magenta  – keywords
const FIELD:  &str = "\x1b[38;5;75m";  // sky-blue – struct field names
const STR:    &str = "\x1b[38;5;114m"; // green    – string / literal values
const INTERP: &str = "\x1b[38;5;81m";  // cyan     – Interpolate
const GLOB:   &str = "\x1b[38;5;208m"; // orange   – Glob
const IDENT:  &str = "\x1b[38;5;255m"; // white    – Ident / plain names
const PUNCT:  &str = "\x1b[38;5;240m"; // grey     – braces, commas, brackets
const NUM:    &str = "\x1b[38;5;220m"; // gold     – numbers
const NONE:   &str = "\x1b[38;5;240m"; // grey     – None

fn indent(depth: usize) -> String {
    "    ".repeat(depth)
}

fn p(color: &str, s: &str) -> String {
    format!("{color}{s}{RST}")
}

fn fmt_shell_part(part: &ShellPart, depth: usize) -> String {
    let ind  = indent(depth);
    let ind1 = indent(depth + 1);
    match part {
        ShellPart::Literal(s) => format!(
            "{ind}{}{PUNCT}({RST}\n\
             {ind1}{STR}\"{s}\"{RST},\n\
             {ind}{PUNCT}){RST}",
            p(IDENT, "Literal"),
        ),
        ShellPart::Interpolate(s) => format!(
            "{ind}{}{PUNCT}({RST}\n\
             {ind1}{INTERP}\"{s}\"{RST},\n\
             {ind}{PUNCT}){RST}",
            p(INTERP, "Interpolate"),
        ),
    }
}

fn fmt_shell_parts(parts: &[ShellPart], depth: usize) -> String {
    let ind = indent(depth);
    if parts.is_empty() {
        return format!("{PUNCT}[]{RST}");
    }
    let items: Vec<String> = parts.iter()
        .map(|p| fmt_shell_part(p, depth + 1))
        .collect();
    format!(
        "{PUNCT}[{RST}\n{items}\n{ind}{PUNCT}]{RST}",
        items = items.iter()
            .map(|s| format!("{s},"))
            .collect::<Vec<_>>()
            .join("\n"),
    )
}

fn fmt_expr(expr: &Expr, depth: usize) -> String {
    let ind  = indent(depth);
    let ind1 = indent(depth + 1);
    match expr {
        Expr::StringLit(s) => format!(
            "{ind}{}{PUNCT}({RST}\n\
             {ind1}{STR}\"{s}\"{RST},\n\
             {ind}{PUNCT}){RST}",
            p(STR, "StringLit"),
        ),
        Expr::Ident(s) => format!(
            "{ind}{}{PUNCT}({RST}\n\
             {ind1}{IDENT}\"{s}\"{RST},\n\
             {ind}{PUNCT}){RST}",
            p(IDENT, "Ident"),
        ),
        Expr::Glob(s) => format!(
            "{ind}{}{PUNCT}({RST}\n\
             {ind1}{GLOB}\"{s}\"{RST},\n\
             {ind}{PUNCT}){RST}",
            p(GLOB, "Glob"),
        ),
        Expr::Array(items) => {
            if items.is_empty() {
                return format!("{ind}{}{PUNCT}([]{RST}){RST}", p(KW, "Array"));
            }
            let inner: Vec<String> = items.iter()
                .map(|e| fmt_expr(e, depth + 1))
                .collect();
            format!(
                "{ind}{}{PUNCT}([{RST}\n{items}\n{ind1}{PUNCT}]{RST}){RST}",
                p(KW, "Array"),
                items = inner.iter()
                    .map(|s| format!("{s},"))
                    .collect::<Vec<_>>()
                    .join("\n"),
                ind1 = ind1,
            )
        }
        Expr::Shell(parts) => format!(
            "{ind}{}{PUNCT}({RST}\n\
             {ind1}{parts_fmt}\n\
             {ind}{PUNCT}){RST}",
            p(STR, "Shell"),
            parts_fmt = fmt_shell_parts(parts, depth + 1),
            ind1 = ind1,
        ),
    }
}

fn fmt_rule(rule: &Rule, depth: usize) -> String {
    let ind  = indent(depth);
    let ind1 = indent(depth + 1);
    let ind2 = indent(depth + 2);

    let depfile_str = match &rule.depfile {
        None => format!("{NONE}None{RST}"),
        Some(parts) => format!(
            "{KW}Some{RST}{PUNCT}({RST}\n{parts}\n{ind1}{PUNCT}){RST}",
            parts = fmt_shell_parts(parts, depth + 2)
                .lines()
                .enumerate()
                .map(|(i, l)| if i == 0 { format!("{ind2}{l}") } else { l.to_string() })
                .collect::<Vec<_>>()
                .join("\n"),
            ind1 = ind1,
        ),
    };

    format!(
        "{ind}{BOLD}{KW}Rule{RST} {PUNCT}{{{RST}\n\
         {ind1}{FIELD}name{RST}{PUNCT}:{RST} {STR}\"{name}\"{RST},\n\
         {ind1}{FIELD}cmd{RST}{PUNCT}:{RST} {cmd},\n\
         {ind1}{FIELD}depfile{RST}{PUNCT}:{RST} {depfile},\n\
         {ind}{PUNCT}}}{RST}",
        name    = rule.name,
        cmd     = fmt_shell_parts(&rule.cmd, depth + 1)
                    .lines()
                    .enumerate()
                    .map(|(i, l)| if i == 0 { l.to_string() } else { format!("{ind1}{l}") })
                    .collect::<Vec<_>>()
                    .join("\n"),
        depfile = depfile_str,
        ind1    = ind1,
    )
}

fn fmt_path_list(paths: &[Vec<ShellPart>], depth: usize) -> String {
    let ind  = indent(depth);
    let ind1 = indent(depth + 1);
    if paths.is_empty() {
        return format!("{PUNCT}[]{RST}");
    }
    let items: Vec<String> = paths.iter()
        .map(|p| {
            let inner = fmt_shell_parts(p, depth + 2);
            let inner_indented = inner.lines()
                .enumerate()
                .map(|(i, l)| if i == 0 { format!("{ind1}{l}") } else { l.to_string() })
                .collect::<Vec<_>>()
                .join("\n");
            format!("{inner_indented},")
        })
        .collect();
    format!("{PUNCT}[{RST}\n{items}\n{ind}{PUNCT}]{RST}", items = items.join("\n"))
}

fn fmt_vars(vars: &[(String, Expr)], depth: usize) -> String {
    let ind  = indent(depth);
    let ind1 = indent(depth + 1);
    if vars.is_empty() {
        return format!("{PUNCT}[]{RST}");
    }
    let items: Vec<String> = vars.iter()
        .map(|(k, v)| {
            let val = fmt_expr(v, depth + 2)
                .lines()
                .map(|l| l.to_string())
                .collect::<Vec<_>>()
                .join("\n");
            format!(
                "{ind1}{PUNCT}({RST}\n\
                 {ind1}    {FIELD}\"{k}\"{RST},\n\
                 {val},\n\
                 {ind1}{PUNCT}){RST},",
            )
        })
        .collect();
    format!("{PUNCT}[{RST}\n{items}\n{ind}{PUNCT}]{RST}", items = items.join("\n"))
}

fn fmt_build_edge(edge: &BuildEdge, depth: usize) -> String {
    let ind  = indent(depth);
    let ind1 = indent(depth + 1);
    format!(
        "{ind}{BOLD}{KW}BuildEdge{RST} {PUNCT}{{{RST}\n\
         {ind1}{FIELD}inputs{RST}{PUNCT}:{RST} {inputs},\n\
         {ind1}{FIELD}outputs{RST}{PUNCT}:{RST} {outputs},\n\
         {ind1}{FIELD}rule{RST}{PUNCT}:{RST} {KW}\"{rule}\"{RST},\n\
         {ind1}{FIELD}vars{RST}{PUNCT}:{RST} {vars},\n\
         {ind}{PUNCT}}}{RST}",
        inputs  = fmt_path_list(&edge.inputs,  depth + 1)
                    .lines().map(|l| l.to_string()).collect::<Vec<_>>().join("\n"),
        outputs = fmt_path_list(&edge.outputs, depth + 1)
                    .lines().map(|l| l.to_string()).collect::<Vec<_>>().join("\n"),
        rule    = edge.rule,
        vars    = fmt_vars(&edge.vars, depth + 1)
                    .lines().map(|l| l.to_string()).collect::<Vec<_>>().join("\n"),
        ind1    = ind1,
    )
}

fn fmt_body(body: &[Stmt], depth: usize) -> String {
    if body.is_empty() {
        return format!("{PUNCT}[]{RST}");
    }
    let ind = indent(depth);
    let stmts: Vec<String> = body.iter()
        .map(|s| fmt_stmt(s, depth + 1))
        .collect();
    format!(
        "{PUNCT}[{RST}\n{items}\n{ind}{PUNCT}]{RST}",
        items = stmts.iter()
            .map(|s| format!("{s},"))
            .collect::<Vec<_>>()
            .join("\n"),
    )
}

fn fmt_stmt(stmt: &Stmt, depth: usize) -> String {
    let ind  = indent(depth);
    let ind1 = indent(depth + 1);
    match stmt {
        Stmt::Target { target, body } => format!(
            "{ind}{BOLD}{KW}Target{RST} {PUNCT}{{{RST}\n\
             {ind1}{FIELD}name{RST}{PUNCT}:{RST} {STR}\"{target}\"{RST},\n\
             {ind1}{FIELD}body{RST}{PUNCT}:{RST} {body_fmt},\n\
             {ind}{PUNCT}}}{RST}",
            body_fmt = fmt_body(body, depth + 1),
            ind1     = ind1,
        ),
        Stmt::Run(name) => format!(
            "{ind}{BOLD}{KW}Run{RST}{PUNCT}({RST}{IDENT}\"{name}\"{RST}{PUNCT}){RST}",
        ),
        Stmt::Rule(rule) => format!(
            "{ind}{BOLD}{KW}Rule{RST}{PUNCT}({RST}\n\
             {rule_fmt},\n\
             {ind}{PUNCT}){RST}",
            rule_fmt = fmt_rule(rule, depth + 1),
        ),
        Stmt::Assign { name, value } => format!(
            "{ind}{BOLD}{KW}Assign{RST} {PUNCT}{{{RST}\n\
             {ind1}{FIELD}name{RST}{PUNCT}:{RST} {STR}\"{name}\"{RST},\n\
             {ind1}{FIELD}value{RST}{PUNCT}:{RST} {value_fmt},\n\
             {ind}{PUNCT}}}{RST}",
            value_fmt = fmt_expr(value, depth + 1).trim_start().to_string(),
            ind1 = ind1,
        ),
        Stmt::Shell(parts) => format!(
            "{ind}{BOLD}{KW}Shell{RST}{PUNCT}({RST}\n\
             {ind1}{parts_fmt},\n\
             {ind}{PUNCT}){RST}",
            parts_fmt = fmt_shell_parts(parts, depth + 1),
            ind1 = ind1,
        ),
        Stmt::For { var, iter, body } => {
            let body_stmts: Vec<String> = body.iter()
                .map(|s| fmt_stmt(s, depth + 2))
                .collect();
            format!(
                "{ind}{BOLD}{KW}For{RST} {PUNCT}{{{RST}\n\
                 {ind1}{FIELD}var{RST}{PUNCT}:{RST} {IDENT}\"{var}\"{RST},\n\
                 {ind1}{FIELD}iter{RST}{PUNCT}:{RST} {iter_fmt},\n\
                 {ind1}{FIELD}body{RST}{PUNCT}:{RST} {PUNCT}[{RST}\n\
                 {body}\n\
                 {ind1}{PUNCT}]{RST},\n\
                 {ind}{PUNCT}}}{RST}",
                iter_fmt = fmt_expr(iter, depth + 1).trim_start().to_string(),
                body = body_stmts.iter()
                    .map(|s| format!("{s},"))
                    .collect::<Vec<_>>()
                    .join("\n"),
                ind1 = ind1,
            )
        }
        Stmt::Build(edge) => format!(
            "{ind}{BOLD}{KW}Build{RST}{PUNCT}({RST}\n\
             {edge_fmt},\n\
             {ind}{PUNCT}){RST}",
            edge_fmt = fmt_build_edge(edge, depth + 1),
        ),
    }
}

pub fn pretty_print_statements(stmts: &[Stmt]) {
    let header = format!(
        "\n{BOLD}{DIM}── {NUM}{count}{RST}{BOLD}{DIM} statement{s} ───────────────────────────{RST}",
        count = stmts.len(),
        s = if stmts.len() == 1 { "" } else { "s" },
    );
    eprintln!("{header}");
    eprintln!("{PUNCT}[{RST}");
    for stmt in stmts {
        eprintln!("{},", fmt_stmt(stmt, 1));
    }
    eprintln!("{PUNCT}]{RST}");
}