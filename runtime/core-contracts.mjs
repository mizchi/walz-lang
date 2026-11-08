// Optional validation for certified shared-memory artifacts.
export function validate(abi, raceFree, memories, options) {
  if (abi === "wlt-core-js-v3" || abi === "wlt-core-js-v4") {
    const contract = abi === "wlt-core-js-v4" ? "wlt-readonly-regions-v1" : "wlt-atomic-cells-v1";
    if (!raceFree || raceFree.hostContract !== contract ||
        typeof raceFree.hostRequired !== "boolean" ||
        raceFree.hostRequired !== (memories.length > 0) ||
        !Array.isArray(raceFree.cells) ||
        !raceFree.cells.every(cell => cell !== null && typeof cell === "object" &&
          Number.isSafeInteger(cell.offset) && cell.offset >= 0 &&
          (cell.width === 4 || cell.width === 8))) {
      throw new Error(`invalid atomic-cell contract for Core JS ABI ${abi.slice(-2)}`);
    }
  }
  if (abi === "wlt-core-js-v4" && (!Array.isArray(raceFree.readonlyRegions) || !raceFree.readonlyRegions.length ||
      !raceFree.readonlyRegions.every(r => Number.isSafeInteger(r.offset) && r.offset >= 0 &&
        Number.isSafeInteger(r.length) && r.length > 0 && r.offset + r.length <= 4294967296 &&
        raceFree.cells.every(c => c.offset + c.width <= r.offset || r.offset + r.length <= c.offset)))) {
    throw new Error("invalid readonly-region contract for Core JS ABI v4");
  }
  const contractOption = abi === "wlt-core-js-v4" ? "readonlyHostContract" : "atomicHostContract";
  if (raceFree?.hostRequired && options[contractOption] !== raceFree.hostContract) {
    throw new Error(`Host shared memory requires explicit ${contractOption}: ${raceFree.hostContract}; all agents must preserve the certified layout and published data`);
  }
}
