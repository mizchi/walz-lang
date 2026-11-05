// Lexical measurements and include inspection, not a gcwat type checker.
function tokens(source) {
  const out = [];
  let i = 0;
  while (i < source.length) {
    if (/\s/u.test(source[i])) {
      i++;
      continue;
    }
    if (source.startsWith(";;", i)) {
      while (i < source.length && source[i] !== "\n") i++;
      continue;
    }
    if (source.startsWith("(;", i)) {
      i += 2;
      let depth = 1;
      while (i < source.length && depth) {
        if (source.startsWith("(;", i)) {
          depth++;
          i += 2;
        } else if (source.startsWith(";)", i)) {
          depth--;
          i += 2;
        } else i++;
      }
      if (depth) throw new Error("unclosed block comment");
      continue;
    }
    if (source[i] === '"') {
      const start = i++;
      let closed = false;
      while (i < source.length) {
        if (source[i] === "\\") {
          i += 2;
          continue;
        }
        if (source[i++] === '"') {
          closed = true;
          break;
        }
      }
      if (!closed) throw new Error("unclosed string");
      out.push({ text: source.slice(start, i) });
      continue;
    }
    if (source[i] === "(" || source[i] === ")") {
      out.push(source[i++]);
      continue;
    }
    const start = i;
    while (i < source.length && !/[\s();"]/u.test(source[i])) i++;
    if (start === i) throw new Error("unexpected source token");
    out.push(source.slice(start, i));
  }
  return out;
}
function tree(items) {
  const stack = [];
  let root;
  for (const item of items) {
    if (item === "(") {
      const next = [];
      if (stack.length) stack.at(-1).push(next);
      else {
        if (root) throw new Error("multiple modules");
        root = next;
      }
      stack.push(next);
      if (stack.length > 256) throw new Error("source nesting limit");
    } else if (item === ")") {
      if (!stack.length) throw new Error("unexpected closing delimiter");
      stack.pop();
    } else {
      if (!stack.length) throw new Error("expected module");
      stack.at(-1).push(item);
    }
  }
  if (stack.length || !root) throw new Error("unclosed module");
  return root;
}
export function assertSnapshotIncludes(source) {
  const root = tree(tokens(source));
  let count = 0;
  if (root[0] !== "module") throw new Error("expected module");
  const walk = (node) => {
    if (!Array.isArray(node)) return;
    if (node[0] === "include") {
      if (node.length !== 2 || node[1]?.text !== '"program.gwat"')
        throw new Error(
          'candidate may only include "program.gwat" from the frozen support',
        );
      count++;
    } else if (node[0] === "module" || node[0] === "namespace")
      node.slice(1).forEach(walk);
  };
  walk(root);
  if (count !== 1) throw new Error('keep exactly one (include "program.gwat")');
}
export function metrics(source) {
  const items = tokens(source),
    root = tree(items);
  let depth = 0,
    maxDepth = 0,
    match = 0,
    patterns = 0,
    stateTypes = 0;
  for (const item of items) {
    if (item === "(") {
      depth++;
      maxDepth = Math.max(maxDepth, depth);
    } else if (item === ")") depth--;
  }
  const text = (node) =>
    Array.isArray(node)
      ? "(" + node.map(text).join(" ") + ")"
      : typeof node === "string"
        ? node
        : node.text;
  const walk = (node) => {
    if (!Array.isArray(node)) return;
    if (node[0] === "match") match++;
    if (node[0] === "let" && Array.isArray(node[1])) patterns++;
    if (node[0] === "pair" && text(node) === "(pair (ref $stock.State) i32)")
      stateTypes++;
    node.forEach(walk);
  };
  walk(root);
  return {
    lines: source.trimEnd().split("\n").length,
    tokens: items.length,
    maxDepth,
    match,
    patternLets: patterns,
    stateTypeOccurrences: stateTypes,
  };
}

export function functionForms(source, name) {
  const root = tree(tokens(source));
  let target = name === "@module" ? root : null;
  const find = (node) => {
    if (!Array.isArray(node)) return;
    if (node[0] === "func" && node[1] === name) target = node;
    node.forEach(find);
  };
  if (!target) find(root);
  if (!target) return null;
  const heads = {};
  const count = (node) => {
    if (!Array.isArray(node)) return;
    if (typeof node[0] === "string") heads[node[0]] = (heads[node[0]] ?? 0) + 1;
    node.forEach(count);
  };
  count(target);
  return heads;
}

export function sourceIncludes(source) {
  const root = tree(tokens(source)),
    paths = [];
  const walk = (node) => {
    if (!Array.isArray(node)) return;
    if (node[0] === "include") {
      if (node.length !== 2 || typeof node[1]?.text !== "string")
        throw new Error("invalid include");
      // Evaluation fixtures use literal relative filenames, not WAT byte escapes.
      paths.push(JSON.parse(node[1].text));
    } else if (node[0] === "module" || node[0] === "namespace")
      node.slice(1).forEach(walk);
  };
  walk(root);
  return paths;
}
