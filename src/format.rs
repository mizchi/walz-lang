//! Token-preserving formatter for both native WAT and gcwat. String escapes and
//! nested comments stay verbatim; only whitespace between tokens is rewritten.
use crate::{Error, ir::Span};

enum Node {
    Token(String),
    Comment(String),
    List(Vec<Node>),
}
struct Reader<'a> {
    source: &'a str,
    offset: usize,
}
impl Reader<'_> {
    fn error(&self, start: usize, message: &str) -> Error {
        Error::at(self.source, Span { start, end: start }, message)
    }
    fn nodes(&mut self, open: Option<usize>, depth: usize) -> Result<Vec<Node>, Error> {
        if depth > 256 {
            return Err(self.error(self.offset, "format nesting limit exceeded"));
        }
        let mut nodes = Vec::new();
        let bytes = self.source.as_bytes();
        while self.offset < bytes.len() {
            let start = self.offset;
            match bytes[start] {
                b if b.is_ascii_whitespace() => self.offset += 1,
                b';' if bytes.get(start + 1) == Some(&b';') => {
                    while self.offset < bytes.len() && bytes[self.offset] != b'\n' {
                        self.offset += 1;
                    }
                    nodes.push(Node::Comment(self.source[start..self.offset].to_owned()));
                }
                b'(' if bytes.get(start + 1) == Some(&b';') => {
                    self.offset += 2;
                    let mut nested = 1;
                    while self.offset < bytes.len() && nested > 0 {
                        if bytes[self.offset..].starts_with(b"(;") {
                            nested += 1;
                            self.offset += 2;
                        } else if bytes[self.offset..].starts_with(b";)") {
                            nested -= 1;
                            self.offset += 2;
                        } else {
                            self.offset += 1;
                        }
                    }
                    if nested != 0 {
                        return Err(self.error(start, "unterminated block comment"));
                    }
                    nodes.push(Node::Comment(self.source[start..self.offset].to_owned()));
                }
                b'(' => {
                    self.offset += 1;
                    nodes.push(Node::List(self.nodes(Some(start), depth + 1)?));
                }
                b')' => {
                    if open.is_none() {
                        return Err(self.error(start, "unexpected ')'"));
                    }
                    self.offset += 1;
                    return Ok(nodes);
                }
                b'"' => {
                    self.offset += 1;
                    let mut closed = false;
                    while self.offset < bytes.len() {
                        let byte = bytes[self.offset];
                        self.offset += 1;
                        if byte == b'"' {
                            closed = true;
                            break;
                        }
                        if byte == b'\\' && self.offset < bytes.len() {
                            self.offset += 1;
                        }
                    }
                    if !closed {
                        return Err(self.error(start, "unterminated string"));
                    }
                    nodes.push(Node::Token(self.source[start..self.offset].to_owned()));
                }
                _ => {
                    while self.offset < bytes.len()
                        && !bytes[self.offset].is_ascii_whitespace()
                        && !matches!(bytes[self.offset], b'(' | b')' | b'"' | b';')
                    {
                        self.offset += 1;
                    }
                    if self.offset == start {
                        return Err(self.error(start, "unexpected token"));
                    }
                    nodes.push(Node::Token(self.source[start..self.offset].to_owned()));
                }
            }
        }
        if let Some(start) = open {
            return Err(self.error(start, "unclosed '('; expected ')'"));
        }
        Ok(nodes)
    }
}
fn flat(n: &Node) -> Option<String> {
    match n {
        Node::Token(t) => Some(t.clone()),
        Node::Comment(_) => None,
        Node::List(items) if matches!(items.first(),Some(Node::Token(t)) if matches!(t.as_str(),"let"|"result.let"|"stream.fold-result"|"match")) => {
            None
        }
        Node::List(items) => Some(format!(
            "({})",
            items
                .iter()
                .map(flat)
                .collect::<Option<Vec<_>>>()?
                .join(" ")
        )),
    }
}
fn head(n: &Node) -> Option<&str> {
    match n {
        Node::List(items) => match items.first()? {
            Node::Token(t) => Some(t),
            _ => None,
        },
        _ => None,
    }
}

fn closing_run(text: &str) -> usize {
    text.bytes().rev().take_while(|c| *c == b')').count()
}

// Header fields form one reading unit even when a type is itself a list.
// This table affects whitespace only; it neither parses nor infers types.
fn header_end(items: &[Node]) -> usize {
    let Some(Node::Token(name)) = items.first() else {
        return 0;
    };
    let meaningful = items
        .iter()
        .filter(|n| !matches!(n, Node::Comment(_)))
        .collect::<Vec<_>>();
    let type_syntax = |n: &&Node| match n {
        Node::Token(_) => true,
        _ => matches!(
            head(n),
            Some("ref" | "option" | "result" | "pair" | "own" | "future" | "stream")
        ),
    };
    let fields = match name.as_str() {
        "result.let" => Some(if meaningful.len() == 4 { 2 } else { 4 }),
        "stream.fold-result" => Some(if meaningful.len() == 5 { 1 } else { 4 }),
        "pair.new" | "result.ok" | "result.err" => Some(
            if meaningful.get(1).is_some_and(type_syntax)
                && meaningful.get(2).is_some_and(type_syntax)
            {
                3
            } else {
                1
            },
        ),
        "case" => Some(3),
        "let" => Some(
            if items
                .iter()
                .filter(|n| !matches!(n, Node::Comment(_)))
                .count()
                == 5
            {
                4
            } else {
                3
            },
        ),
        "match" => Some(2),
        _ => None,
    };
    if let Some(fields) = fields {
        let mut count = 0;
        let mut end = 0;
        for item in items {
            end += 1;
            if !matches!(item, Node::Comment(_)) {
                count += 1;
            }
            if count == fields {
                break;
            }
        }
        if name == "match" {
            let mut next = end;
            while matches!(items.get(next), Some(Node::Comment(_))) {
                next += 1;
            }
            if items.get(next).is_some_and(|n| head(n) == Some("result")) {
                end = next + 1;
            }
        }
        return end;
    }
    if matches!(name.as_str(), "func" | "closure") {
        let mut end = 1;
        if matches!(items.get(end),Some(Node::Token(t)) if t.starts_with('$')) {
            end += 1;
        }
        while let Some(item) = items.get(end) {
            if matches!(item, Node::Comment(_))
                || matches!(
                    head(item),
                    Some(
                        "param"
                            | "result"
                            | "effects"
                            | "export"
                            | "import"
                            | "capture"
                            | "type"
                            | "local"
                    )
                )
            {
                end += 1;
            } else {
                break;
            }
        }
        return end;
    }
    items
        .iter()
        .take_while(|n| matches!(n, Node::Token(_)))
        .count()
}

fn write_header(items: &[Node], indent: usize, output: &mut String) {
    output.push_str(&" ".repeat(indent));
    output.push('(');
    let mut column = indent + 1;
    for (index, item) in items.iter().enumerate() {
        if let Some(text) = flat(item).filter(|t| closing_run(t) <= 4) {
            if column > 0 && index > 0 && column + 1 + text.len() > 88 {
                output.push('\n');
                column = 0;
            }
            if column == 0 {
                output.push_str(&" ".repeat(indent + 2));
                column = indent + 2;
            } else if index > 0 {
                output.push(' ');
                column += 1;
            }
            output.push_str(&text);
            column += text.len();
        } else {
            if column > 0 {
                output.push('\n');
            }
            write(item, indent + 2, output);
            column = 0;
        }
    }
    if column > 0 {
        output.push('\n');
    }
}

fn write(n: &Node, indent: usize, output: &mut String) {
    let padding = " ".repeat(indent);
    if let Some(s) = flat(n)
        && s.len() + indent <= 88
        && closing_run(&s) <= 4
    {
        output.push_str(&padding);
        output.push_str(&s);
        output.push('\n');
        return;
    }
    match n {
        Node::Token(t) | Node::Comment(t) => {
            output.push_str(&padding);
            output.push_str(t);
            output.push('\n');
        }
        Node::List(items) => {
            let prefix = header_end(items);
            write_header(&items[..prefix], indent, output);
            for (index, item) in items[prefix..].iter().enumerate() {
                if index > 0
                    && matches!(head(n), Some("module" | "namespace"))
                    && matches!(head(item), Some("func" | "closure"))
                    && matches!(head(&items[prefix + index - 1]), Some("func" | "closure"))
                {
                    output.push('\n');
                }
                write(item, indent + 2, output);
            }
            if matches!(items.last(), Some(Node::Comment(_)))
                || closing_run(output.trim_end_matches('\n')) >= 4
            {
                output.push_str(&padding);
            } else {
                output.pop(); // Append to the final expression, not a line comment.
            }
            output.push_str(")\n");
        }
    }
}
pub fn source(source: &str) -> Result<String, Error> {
    let nodes = Reader { source, offset: 0 }.nodes(None, 0)?;
    let mut output = String::new();
    for node in &nodes {
        write(node, 0, &mut output);
    }
    Ok(output)
}
