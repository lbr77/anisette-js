// HTTP client abstraction — allows swapping fetch vs Node.js http in tests

export interface HttpClient {
  get(url: string, headers: Record<string, string>): Promise<Uint8Array>;
  post(
    url: string,
    body: string,
    headers: Record<string, string>
  ): Promise<Uint8Array>;
}

export class FetchHttpClient implements HttpClient {
  async get(url: string, headers: Record<string, string>): Promise<Uint8Array> {
    const response = await fetch(url, { method: "GET", headers });
    if (!response.ok) {
      throw new Error(`HTTP GET ${url} failed: ${response.status} ${response.statusText}`);
    }
    return new Uint8Array(await response.arrayBuffer());
  }

  async post(
    url: string,
    body: string,
    headers: Record<string, string>
  ): Promise<Uint8Array> {
    const response = await fetch(url, { method: "POST", body, headers });
    if (!response.ok) {
      throw new Error(`HTTP POST ${url} failed: ${response.status} ${response.statusText}`);
    }
    return new Uint8Array(await response.arrayBuffer());
  }
}
