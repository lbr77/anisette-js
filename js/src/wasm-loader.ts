// @ts-expect-error — glue file is generated, no types available
import ModuleFactory from "../../dist/anisette_rs.node.js";

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export async function loadWasm(moduleOverrides?: Record<string, any>): Promise<any> {
  return ModuleFactory({ ...moduleOverrides });
}
