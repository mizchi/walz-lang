import { parseExpressionAt } from "acorn";

const expression = Symbol("Walz table expression");
const descending = Symbol("Walz descending order");
const aggregates = { count: 100, sum: 101, mean: 102, average: 102, min: 103, max: 104, valid: 105, invalid: 106, distinct: 107 };
const binary = { "+": 10, "-": 11, "*": 12, "/": 13, "===": 20, "!==": 21, "<": 22, "<=": 23, ">": 24, ">=": 25, "==": 26, "!=": 27, "&&": 30, "||": 31, "??": 35 };
const literal = value => Object.freeze({ [expression]: true, literal: value });
const field = name => Object.freeze({ [expression]: true, field: name });
function namespaceRoot(value) {
  if (value.type === "Identifier") return value.name;
  return value.type === "MemberExpression" && !value.optional ? namespaceRoot(value.object) : undefined;
}
export const op = Object.freeze(Object.fromEntries(Object.entries(aggregates).map(([name, code]) => [name, (...args) => {
  if (args.length > 1) throw new TypeError(`op.${name} accepts at most one argument`);
  const arg = args[0];
  return Object.freeze({ [expression]: true, code, args: [typeof arg === "string" ? field(arg) : arg === undefined ? literal(undefined) : arg] });
}])));
export const desc = value => Object.freeze({ [descending]: true, value });
export const order = value => value?.[descending] ? { value: value.value, descending: 1 } : { value, descending: 0 };

export function compile(value, names, env, { aggregate = false, column = false } = {}) {
  const { api, cell } = env, empty = api.literal(cell(undefined));
  const columnAt = (name, insideAggregate = false) => {
    if (aggregate && !insideAggregate) throw new TypeError("Rollup column references must be inputs to an aggregate");
    const index = names.indexOf(name); if (index < 0) throw new ReferenceError(`Unknown table column: ${name}`); return api.field(index);
  };
  const node = (code, args) => api.node(code, args[0] ?? empty, args[1] ?? empty, args[2] ?? empty);
  const bindings = new Map(); let parameter;
  function visit(ast, nestedAggregate = false) {
    if (ast.type === "Literal") {
      if (ast.regex || ast.bigint) throw new TypeError("Regular expressions and bigint table expressions are not supported");
      return api.literal(cell(ast.value));
    }
    if (ast.type === "Identifier") {
      if (bindings.has(ast.name)) return columnAt(bindings.get(ast.name), nestedAggregate);
      if (ast.name === "undefined") return api.literal(cell(undefined));
      if (ast.name === "NaN") return api.literal(cell(NaN));
      if (ast.name === "Infinity") return api.literal(cell(Infinity));
      throw new ReferenceError(`Unbound table expression identifier: ${ast.name}; use columns or literal values`);
    }
    if (ast.type === "MemberExpression" && !ast.optional && ast.object.type === "Identifier" && ast.object.name === parameter) {
      const name = ast.computed ? ast.property.type === "Literal" ? ast.property.value : undefined : ast.property.name;
      if (typeof name !== "string") throw new TypeError("Column references must use a static column name");
      return columnAt(name, nestedAggregate);
    }
    if (ast.type === "UnaryExpression" && ["!", "-", "+"].includes(ast.operator))
      return node({ "!": 32, "-": 33, "+": 36 }[ast.operator], [visit(ast.argument, nestedAggregate)]);
    if (["BinaryExpression", "LogicalExpression"].includes(ast.type) && binary[ast.operator] !== undefined)
      return node(binary[ast.operator], [visit(ast.left, nestedAggregate), visit(ast.right, nestedAggregate)]);
    if (ast.type === "ConditionalExpression") return node(34, [visit(ast.test, nestedAggregate), visit(ast.consequent, nestedAggregate), visit(ast.alternate, nestedAggregate)]);
    if (ast.type === "CallExpression" && !ast.optional && ast.callee.type === "MemberExpression" && !ast.callee.computed &&
      namespaceRoot(ast.callee.object) && namespaceRoot(ast.callee.object) !== parameter && !bindings.has(namespaceRoot(ast.callee.object))) {
      const name = ast.callee.property.name, code = Object.hasOwn(aggregates, name) ? aggregates[name] : undefined;
      if (code === undefined) throw new TypeError(`Unsupported table operation: op.${name}`);
      if (!aggregate || nestedAggregate) throw new TypeError("Aggregates are supported in rollup; nested aggregates are not supported");
      if (ast.arguments.length > 1) throw new TypeError(`op.${name} accepts at most one argument`);
      return node(code, [ast.arguments.length ? visit(ast.arguments[0], true) : empty]);
    }
    throw new TypeError(`Unsupported table expression: ${ast.type}${ast.operator ? ` (${ast.operator})` : ""}`);
  }
  if (value?.[expression]) {
    function lower(v, nested = false) {
      if (!v?.[expression]) return compile(v, names, env, { aggregate: false });
      if (Object.hasOwn(v, "literal")) return api.literal(cell(v.literal));
      if (Object.hasOwn(v, "field")) return columnAt(v.field, nested);
      if (!aggregate || nested) throw new TypeError("Aggregates are supported in rollup; nested aggregates are not supported");
      return node(v.code, v.args.map(a => lower(a, true)));
    }
    return lower(value);
  }
  if (column && typeof value === "string") return columnAt(value);
  if (typeof value === "string") throw new TypeError("Use an arrow expression for a string literal; expression source strings are not supported");
  if (typeof value !== "function") return api.literal(cell(value));
  const source = `(${Function.prototype.toString.call(value)})`;
  let fn;
  try { fn = parseExpressionAt(source, 0, { ecmaVersion: "latest" }); }
  catch (error) { throw new SyntaxError(`Invalid table expression: ${error.message}`); }
  if (!["ArrowFunctionExpression", "FunctionExpression"].includes(fn.type) || fn.async || fn.generator || fn.params.length > 1)
    throw new TypeError("Use a synchronous table expression with zero or one row parameter");
  const param = fn.params[0];
  if (param?.type === "Identifier") parameter = param.name;
  else if (param?.type === "ObjectPattern") {
    for (const p of param.properties) {
      if (p.type !== "Property" || p.computed || p.value.type !== "Identifier") throw new TypeError("Use simple column destructuring");
      bindings.set(p.value.name, p.key.name ?? p.key.value);
    }
  } else if (param) throw new TypeError("Unsupported table parameter binding");
  let body = fn.body;
  if (body.type === "BlockStatement") {
    if (body.body.length !== 1 || body.body[0].type !== "ReturnStatement" || !body.body[0].argument)
      throw new TypeError("A table expression block must contain one return expression");
    body = body.body[0].argument;
  }
  return visit(body);
}
