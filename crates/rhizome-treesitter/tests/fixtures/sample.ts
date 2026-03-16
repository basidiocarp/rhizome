import { readFileSync } from "fs";

const MAX_RETRIES = 3;

interface Config {
  host: string;
  port: number;
}

class HttpClient {
  private baseUrl: string;

  constructor(config: Config) {
    this.baseUrl = `${config.host}:${config.port}`;
  }

  async fetch(path: string): Promise<Response> {
    return globalThis.fetch(`${this.baseUrl}${path}`);
  }
}

function createClient(config: Config): HttpClient {
  return new HttpClient(config);
}
