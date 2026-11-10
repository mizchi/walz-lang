// Lexical measurements and include inspection, not a walz type checker.
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
export function assertSnapshotIncludes(source, corePackage = 'walz:core') {
  const root = tree(tokens(source));
  let count = 0;
  if (root[0] !== "module") throw new Error("expected module");
  const walk = (node) => {
    if (!Array.isArray(node)) return;
    if (["include", "@include"].includes(node[0])) {
      if (node.length !== 2 || node[1]?.text !== '"program.walz"')
        throw new Error(
          'candidate may only include "program.walz" from the frozen support',
        );
      count++;
    } else if (["import", "@import", "@use"].includes(node[0]) && node[1]?.text && !node[2]?.text) {
      if (node[1].text !== JSON.stringify(corePackage)) throw new Error(`candidate may only import ${corePackage} in addition to frozen support`);
    } else if (node[0] === "module" || ["namespace", "@namespace"].includes(node[0]))
      node.slice(1).forEach(walk);
  };
  walk(root);
  if (count !== 1) throw new Error('keep exactly one (include "program.walz")');
}
export function metrics(source) {
  const items = tokens(source),
    root = tree(items);
  let depth = 0,
    maxDepth = 0,
    match = 0,
    patterns = 0,
    grouped = 0,
    stateTypes = 0;
  for (const item of items) {
    if (item === "(") {
      depth++;
      maxDepth = Math.max(maxDepth, depth);
    } else if (item === ")") depth--;
  }
  const canonical = (name) => typeof name === "string" ? name.replace(/^@/, "") : name;
  const text = (node) =>
    Array.isArray(node)
      ? "(" + node.map(text).join(" ") + ")"
      : typeof node === "string"
        ? canonical(node)
        : node.text;
  const walk = (node) => {
    if (!Array.isArray(node)) return;
    if (canonical(node[0]) === "match") match++;
    if (canonical(node[0]) === "let" && Array.isArray(node[1])) {
      if (Array.isArray(node[1][0])) grouped++;
      else patterns++;
    }
    if ((["pair", "tuple"].includes(canonical(node[0])) && ["(pair (ref $stock.State) i32)", "(tuple (ref $stock.State) i32)"].includes(text(node))) ||
        (canonical(node[0]) === "inst" && ["(inst $pair (ref $stock.State) i32)", "(inst $tuple (ref $stock.State) i32)"].includes(text(node))))
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
    groupedLets: grouped,
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
    if (typeof node[0] === "string") {
      // Count the same checked operation across historical/public spellings.
      const head = node[0].replace(/^@/, "").replace(
        /^(string|tuple|box)(?=\.|$)/,
        family => ({ string: "text", tuple: "pair", box: "own" })[family],
      );
      heads[head] = (heads[head] ?? 0) + 1;
      if (head === "let" && Array.isArray(node[1]?.[0])) heads["let.group"] = (heads["let.group"] ?? 0) + 1;
    }
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
    if (["include", "@include"].includes(node[0])) {
      if (node.length !== 2 || typeof node[1]?.text !== "string")
        throw new Error("invalid include");
      // Evaluation fixtures use literal relative filenames, not WAT byte escapes.
      paths.push(JSON.parse(node[1].text));
    } else if (["@use", "@import", "import"].includes(node[0]) && node[1]?.text && !node[2]?.text) {
      const path = JSON.parse(node[1].text);
      if (!path.startsWith("walz:")) paths.push(path);
    } else if (node[0] === "module" || ["namespace", "@namespace"].includes(node[0]))
      node.slice(1).forEach(walk);
  };
  walk(root);
  return paths;
}
