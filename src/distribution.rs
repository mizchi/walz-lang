//! Embedded Core/JSPI distribution, shared by temporary run and public build.
use crate::component::core_js::CoreJs;
use std::path::Path;

pub const FILES: &[&str] = &[
    "core.wasm",
    "memory.wasm",
    "bindings.mjs",
    "core.mjs",
    "core-contracts.mjs",
    "scopes.mjs",
    "readables.mjs",
    "host.mjs",
    "tasks.mjs",
    "resources.mjs",
    "children.mjs",
    "channels.mjs",
    "manifest.json",
    "package.json",
    "index.js",
    "wasi-filesystem.mjs",
    "node-filesystem.mjs",
];

pub fn write(output: &Path, built: &CoreJs) -> Result<(), Box<dyn std::error::Error>> {
    std::fs::create_dir_all(output)?;
    for (name, bytes) in [
        ("core.wasm", built.wasm.as_slice()),
        ("memory.wasm", built.memory.as_slice()),
        ("bindings.mjs", built.bindings.as_bytes()),
        ("core.mjs", include_bytes!("../runtime/core.mjs").as_slice()),
        (
            "core-contracts.mjs",
            include_bytes!("../runtime/core-contracts.mjs").as_slice(),
        ),
        (
            "scopes.mjs",
            include_bytes!("../runtime/scopes.mjs").as_slice(),
        ),
        (
            "readables.mjs",
            include_bytes!("../runtime/readables.mjs").as_slice(),
        ),
        ("host.mjs", include_bytes!("../runtime/host.mjs").as_slice()),
        (
            "tasks.mjs",
            include_bytes!("../runtime/tasks.mjs").as_slice(),
        ),
        (
            "resources.mjs",
            include_bytes!("../runtime/resources.mjs").as_slice(),
        ),
        (
            "children.mjs",
            include_bytes!("../runtime/children.mjs").as_slice(),
        ),
        (
            "channels.mjs",
            include_bytes!("../runtime/channels.mjs").as_slice(),
        ),
        (
            "wasi-filesystem.mjs",
            include_bytes!("../runtime/wasi-filesystem.mjs").as_slice(),
        ),
        (
            "node-filesystem.mjs",
            include_bytes!("../runtime/node-filesystem.mjs").as_slice(),
        ),
    ] {
        let needed = match name {
            "core-contracts.mjs" => !built.manifest["raceFree"].is_null(),
            "scopes.mjs" => built.manifest["asyncScopes"] == true,
            "readables.mjs" => built.manifest["readables"] == true,
            "children.mjs" => built.manifest["childTasks"] == true,
            "channels.mjs" => built.manifest["imports"]
                .as_array()
                .is_some_and(|imports| imports.iter().any(|b| b["capability"] == "channels")),
            "wasi-filesystem.mjs" | "node-filesystem.mjs" => {
                built.manifest["imports"].as_array().is_some_and(|imports| {
                    imports
                        .iter()
                        .any(|b| b["module"] == "wlt:filesystem/files@0.3.0")
                })
            }
            _ => true,
        };
        if !needed {
            continue;
        }
        std::fs::write(output.join(name), bytes)?;
    }
    std::fs::write(
        output.join("manifest.json"),
        serde_json::to_vec_pretty(&built.manifest)?,
    )?;
    std::fs::write(
        output.join("package.json"),
        "{\"type\":\"module\",\"private\":true}\n",
    )?;
    let children = built.manifest["childTasks"] == true;
    let readables = built.manifest["readables"] == true;
    let contract = !built.manifest["raceFree"].is_null();
    std::fs::write(
        output.join("index.js"),
        format!(
            "{scopes_export}{children_import}{readables_import}{contract_import}import {{instantiate as create}} from './core.mjs';\nimport {{bind}} from './bindings.mjs';\nexport {{TaskCancelledError,TaskTimeoutError}} from './tasks.mjs';\nexport const instantiate=(options={{}})=>create(import.meta.url,{abi},bind,options,{imports},{memories},{race},{children},{readables},{contract});\n",
            scopes_export = if built.manifest["asyncScopes"] == true {
                "export {withTaskScope} from './scopes.mjs';\n"
            } else {
                ""
            },
            children_import = if children {
                "import {Children} from './children.mjs';\n"
            } else {
                ""
            },
            readables_import = if readables {
                "import {install} from './readables.mjs';\n"
            } else {
                ""
            },
            contract_import = if contract {
                "import {validate} from './core-contracts.mjs';\n"
            } else {
                ""
            },
            abi = built.manifest["abi"],
            imports = built.manifest["imports"],
            memories = built
                .manifest
                .get("memories")
                .unwrap_or(&serde_json::json!([])),
            race = built
                .manifest
                .get("raceFree")
                .unwrap_or(&serde_json::Value::Null),
            children = if children { "Children" } else { "null" },
            readables = if readables { "install" } else { "null" },
            contract = if contract { "validate" } else { "null" },
        ),
    )?;
    Ok(())
}
