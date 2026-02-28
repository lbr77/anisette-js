// Libcurl-based HTTP client for browser

import type { HttpClient } from "anisette-js";
import { libcurl } from "./libcurl-init";

export class LibcurlHttpClient implements HttpClient {
  async get(url: string, headers: Record<string, string>): Promise<Uint8Array> {
    // @ts-ignore
    const response = (await libcurl.fetch(url, { method: "GET", headers, insecure: true })) as Response;
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
    // @ts-ignore
    const response = (await libcurl.fetch(url, { method: "POST", body, headers, insecure: true})) as Response;
    if (!response.ok) {
      throw new Error(`HTTP POST ${url} failed: ${response.status} ${response.statusText}`);
    }
    return new Uint8Array(await response.arrayBuffer());
  }
}