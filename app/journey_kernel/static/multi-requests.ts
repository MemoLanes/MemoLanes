/**
 * MultiRequest Module
 *
 * A unified request class that routes through standard fetch().
 * In production, requests to custom scheme (iOS) or intercepted URL (Android)
 * are handled transparently by the native WebView interceptor.
 * In dev mode, requests go to a real HTTP server.
 */

/**
 * Unified request format matching Rust Request struct
 */
interface UnifiedRequest {
  requestId: string;
  query: string;
  payload: Record<string, any>;
}

/**
 * Unified response format
 */
interface UnifiedResponse {
  requestId: string;
  [key: string]: any;
}

/**
 * Fetch options
 */
interface FetchOptions extends RequestInit {
  timeout?: number;
}

/**
 * Status information
 */
interface StatusInfo {
  endpoint: string | null;
  pendingRequests: number;
  isHttpMode: boolean;
}

/**
 * Extended window interface
 */
declare global {
  interface Window {
    [key: string]: any;
    MultiRequest?: typeof MultiRequest;
  }
}

/**
 * MultiRequest Class
 *
 * Provides a unified interface for making requests via fetch().
 * For intercepted endpoints (memolanes://, https://memolanes.local/),
 * uses GET with query parameters (native interceptor handles these).
 * For HTTP endpoints (dev server), uses POST with JSON body.
 */
class MultiRequest {
  private cgiEndpoint: string | null;
  private requestId: number;
  // private timeout: number;

  constructor(cgiEndpoint: string | null = null) {
    this.cgiEndpoint = cgiEndpoint;
    this.requestId = 0;
    // this.timeout = 30000;
  }

  /**
   * Set the CGI endpoint
   */
  setEndpoint(cgiEndpoint: string | null): void {
    this.cgiEndpoint = cgiEndpoint;
  }

  /**
   * Set timeout for requests
   */
  // setTimeout(timeout: number): void {
  //   this.timeout = timeout;
  // }

  private isInterceptedEndpoint(): boolean {
    if (!this.cgiEndpoint) return false;
    return (
      this.cgiEndpoint.startsWith("memolanes://") ||
      this.cgiEndpoint.startsWith("https://memolanes.local/")
    );
  }

  /**
   * Main fetch method - routes to GET (intercepted) or POST (HTTP)
   */
  async fetch(
    resource: string,
    params: Record<string, any> | null = null,
    options: FetchOptions = {},
  ): Promise<UnifiedResponse> {
    if (!this.cgiEndpoint) {
      throw new Error("No CGI endpoint set. Call setEndpoint() first.");
    }

    if (this.isInterceptedEndpoint()) {
      return this.fetchViaInterceptor(resource, params, options);
    } else {
      return this.fetchViaHttp(resource, params, options);
    }
  }

  /**
   * Intercepted endpoint: GET with query parameters.
   * The native WebView interceptor handles custom scheme (iOS) or
   * URL pattern (Android) and returns the response directly.
   */
  async fetchViaInterceptor(
    resource: string,
    params: Record<string, any> | null = null,
    options: FetchOptions = {},
  ): Promise<UnifiedResponse> {
    const url = new URL(`${this.cgiEndpoint}/${resource}`);
    if (params) {
      for (const [key, value] of Object.entries(params)) {
        if (value !== undefined && value !== null) {
          url.searchParams.set(key, String(value));
        }
      }
    }

    const response = await fetch(url.toString(), {
      cache: "no-cache",
      ...options,
    });

    if (response.ok) {
      return await response.json();
    } else {
      throw new Error(`HTTP ${response.status}: ${response.statusText}`);
    }
  }

  /**
   * HTTP fetch implementation - POST with JSON body (for dev server)
   */
  async fetchViaHttp(
    resource: string,
    params: Record<string, any> | null = null,
    options: FetchOptions = {},
  ): Promise<UnifiedResponse> {
    const url = `${this.cgiEndpoint}/api`;

    const requestId = `http_${this.requestId++}_${Date.now()}`;

    const unifiedRequest: UnifiedRequest = {
      requestId: requestId,
      query: resource,
      payload: params || {},
    };

    const fetchOptions: RequestInit = {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        ...(options.headers || {}),
      },
      body: JSON.stringify(unifiedRequest),
      cache: "no-cache",
      ...options,
    };

    const response = await fetch(url, fetchOptions);

    if (response.ok) {
      const jsonResponse: UnifiedResponse = await response.json();
      return jsonResponse;
    } else {
      throw new Error(`HTTP ${response.status}: ${response.statusText}`);
    }
  }

  /**
   * Raw fetch for intercepted endpoints - returns the Response object directly
   * for binary consumption (arrayBuffer(), etc).
   * For HTTP dev server endpoints, falls back to POST and returns the Response.
   */
  async fetchRaw(
    resource: string,
    params: Record<string, any> | null = null,
    options: FetchOptions = {},
  ): Promise<Response> {
    if (!this.cgiEndpoint) {
      throw new Error("No CGI endpoint set. Call setEndpoint() first.");
    }

    if (this.isInterceptedEndpoint()) {
      const url = new URL(`${this.cgiEndpoint}/${resource}`);
      if (params) {
        for (const [key, value] of Object.entries(params)) {
          if (value !== undefined && value !== null) {
            url.searchParams.set(key, String(value));
          }
        }
      }
      return fetch(url.toString(), { cache: "no-cache", ...options });
    } else {
      const url = `${this.cgiEndpoint}/api`;
      const requestId = `http_${this.requestId++}_${Date.now()}`;
      const unifiedRequest: UnifiedRequest = {
        requestId,
        query: resource,
        payload: params || {},
      };
      return fetch(url, {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
          ...(options.headers || {}),
        },
        body: JSON.stringify(unifiedRequest),
        cache: "no-cache",
        ...options,
      });
    }
  }

  /**
   * Get status information
   */
  getStatus(): StatusInfo {
    return {
      endpoint: this.cgiEndpoint,
      pendingRequests: 0,
      isHttpMode:
        this.cgiEndpoint !== null && !this.isInterceptedEndpoint(),
    };
  }

  /**
   * Clear all pending requests (no-op, kept for API compatibility)
   */
  clearPending(): void {
    // No pending requests to clear with standard fetch
  }
}

export { MultiRequest };
export default MultiRequest;

if (typeof window !== "undefined") {
  window.MultiRequest = MultiRequest;
}
