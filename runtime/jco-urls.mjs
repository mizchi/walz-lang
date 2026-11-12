// Keep the pinned transpiler's browser fallback statically bounded for bundlers.
// A template URL like `./${name}` makes Vite include every sibling as an asset.
export function bindCoreModuleUrls(source, modules) {
  const before = 'if (!getCoreModule) getCoreModule = (name) => fetchCompile(new URL(`./${name}`, import.meta.url));';
  if (source.split(before).length !== 2)
    throw new Error("jco core-module URL fallback changed; review the pinned transpiler");
  const entries = modules.map(name => `${JSON.stringify(name)}: new URL(${JSON.stringify("./" + name)}, import.meta.url)`).join(",");
  return source.replace(before, `if (!getCoreModule) {
      const urls = {${entries}};
      getCoreModule = (name) => fetchCompile(urls[name]);
    }`);
}
