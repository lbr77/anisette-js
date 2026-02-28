import { libcurl } from "../public/libcurl_full.mjs";

let initialized = false;
let initPromise: Promise<void> | null = null;

export async function initLibcurl(): Promise<void> {
  if (initialized) return;
  if (initPromise) return initPromise;
  initPromise = (async () => {
    const wsProto = location.protocol === "https:" ? "wss:" : "ws:";
    let wsUrl = `${wsProto}//${location.host}/wisp/`;
    libcurl.set_websocket(wsUrl);
    await libcurl.load_wasm("/libcurl.wasm");
    initialized = true;
  })();

  return initPromise;
}

export { libcurl };