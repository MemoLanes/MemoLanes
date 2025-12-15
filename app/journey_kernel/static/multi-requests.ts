/**
 * MultiRequest Module
 *
 * A unified TypeScript class for handling both HTTP and Flutter IPC communications
 * with a consistent API.
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
 * Pending request info
 */
interface PendingRequest {
  resolve: (value: UnifiedResponse) => void;
  reject: (reason: Error) => void;
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
  channelName: string | null;
  channelAvailable: boolean;
  pendingRequests: number;
  isFlutterMode: boolean;
  isHttpMode: boolean;
}

/**
 * Flutter channel interface
 */
interface FlutterChannel {
  postMessage(message: string): void;
}

/**
 * Extended window interface for Flutter channels
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
 * Provides a unified interface for making both HTTP requests and Flutter IPC calls.
 * Automatically routes requests based on the endpoint protocol (http:// vs flutter://).
 */
class MultiRequest {
  private cgiEndpoint: string | null;
  private requestId: number;
  private pendingRequests: Map<number, PendingRequest>;
  private channelName: string | null;
  private channel: FlutterChannel | null;
  private timeout: number;

  constructor(cgiEndpoint: string | null = null) {
    this.cgiEndpoint = cgiEndpoint;
    this.requestId = 0;
    this.pendingRequests = new Map<number, PendingRequest>();
    this.channelName = null;
    this.channel = null;
    this.timeout = 30000; // Default 30 second timeout

    // Parse flutter:// endpoints
    if (cgiEndpoint && cgiEndpoint.startsWith("flutter://")) {
      this.channelName = cgiEndpoint.replace("flutter://", "");
      this.channel = window[this.channelName] as FlutterChannel;

      // Set up response handler for this specific instance
      this._setupResponseHandler();
    }
  }

  /**
   * Set up response handler for Flutter IPC for this specific instance
   */
  private _setupResponseHandler(): void {
    if (!this.channelName) return;

    // Set up JSON response handler
    const jsonHandlerName = `handle_${this.channelName}_JsonResponse`;
    if (!window[jsonHandlerName]) {
      window[jsonHandlerName] = (responseJson: UnifiedResponse): void => {
        this._handleJsonResponse(responseJson);
      };
    }
  }

  /**
   * Handle object IPC response for this instance
   */
  private _handleJsonResponse(responseData: UnifiedResponse): void {
    try {
      // Directly use the object response (no parsing needed)
      const jsonResponse = responseData;
      // const responseSize = (JSON.stringify(responseData).length / 1024).toFixed(2);
      // console.log(`MultiRequest: Direct object response: ${responseSize} KB`);

      const requestId = parseInt(jsonResponse.requestId);

      if (!this.pendingRequests.has(requestId)) {
        // This response might be for a different instance
        return;
      }

      const pendingRequest = this.pendingRequests.get(requestId);
      if (!pendingRequest) return;

      const { resolve, reject } = pendingRequest;

      try {
        // Return the JSON response directly
        resolve(jsonResponse);
      } catch (error) {
        reject(error as Error);
      } finally {
        this.pendingRequests.delete(requestId);
      }
    } catch (error) {
      console.error("Failed to process response:", error);
      console.error("Raw response:", responseData);
    }
  }

  /**
   * Set the CGI endpoint for this instance
   */
  setEndpoint(cgiEndpoint: string | null): void {
    this.cgiEndpoint = cgiEndpoint;

    // Re-parse if it's a flutter endpoint
    if (cgiEndpoint && cgiEndpoint.startsWith("flutter://")) {
      this.channelName = cgiEndpoint.replace("flutter://", "");
      this.channel = window[this.channelName] as FlutterChannel;
      this._setupResponseHandler();
    } else {
      this.channelName = null;
      this.channel = null;
    }
  }

  /**
   * Set timeout for requests
   */
  setTimeout(timeout: number): void {
    this.timeout = timeout;
  }

  /**
   * Main fetch method that routes to HTTP or Flutter IPC
   */
  async fetch(
    resource: string,
    params: Record<string, any> | null = null,
    options: FetchOptions = {},
  ): Promise<UnifiedResponse> {
    if (!this.cgiEndpoint) {
      throw new Error("No CGI endpoint set. Call setEndpoint() first.");
    }

    if (this.cgiEndpoint.startsWith("flutter://")) {
      return this.fetchViaFlutter(resource, params, options);
    } else {
      return this.fetchViaHttp(resource, params, options);
    }
  }

  /**
   * HTTP fetch implementation - Always uses unified JSON API
   */
  async fetchViaHttp(
    resource: string,
    params: Record<string, any> | null = null,
    options: FetchOptions = {},
  ): Promise<UnifiedResponse> {
    const url = `${this.cgiEndpoint}/api`;

    // Generate unique request ID
    const requestId = `http_${this.requestId++}_${Date.now()}`;

    // Create unified request payload matching Rust Request struct
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
      // Return the unified response directly
      return jsonResponse;
    } else {
      throw new Error(`HTTP ${response.status}: ${response.statusText}`);
    }
  }

  /**
   * Flutter IPC fetch implementation
   */
  async fetchViaFlutter(
    resource: string,
    params: Record<string, any> | null = null,
    options: FetchOptions = {},
  ): Promise<UnifiedResponse> {
    if (!this.channel) {
      throw new Error(`Flutter channel '${this.channelName}' not available`);
    }

    return new Promise<UnifiedResponse>((resolve, reject) => {
      const requestId = ++this.requestId;

      // Store request info
      this.pendingRequests.set(requestId, {
        resolve,
        reject,
      });

      // Send unified request format via JavaScript channel
      try {
        const unifiedRequest: UnifiedRequest = {
          requestId: requestId.toString(),
          query: resource,
          payload: params || {},
        };

        this.channel!.postMessage(JSON.stringify(unifiedRequest));
      } catch (error) {
        this.pendingRequests.delete(requestId);
        const errorMessage =
          error instanceof Error ? error.message : String(error);
        reject(
          new Error(`Failed to send Flutter IPC request: ${errorMessage}`),
        );
        return;
      }

      // Set timeout
      const timeoutMs = options.timeout || this.timeout;
      setTimeout(() => {
        if (this.pendingRequests.has(requestId)) {
          this.pendingRequests.delete(requestId);
          reject(new Error(`Flutter IPC request timeout after ${timeoutMs}ms`));
        }
      }, timeoutMs);
    });
  }

  /**
   * Get status information for this instance
   */
  getStatus(): StatusInfo {
    return {
      endpoint: this.cgiEndpoint,
      channelName: this.channelName,
      channelAvailable: !!this.channel,
      pendingRequests: this.pendingRequests.size,
      isFlutterMode:
        this.cgiEndpoint !== null && this.cgiEndpoint.startsWith("flutter://"),
      isHttpMode:
        this.cgiEndpoint !== null && !this.cgiEndpoint.startsWith("flutter://"),
    };
  }

  /**
   * Clear all pending requests (useful for cleanup)
   */
  clearPending(): void {
    for (const [_, { reject }] of this.pendingRequests) {
      reject(new Error("Request cancelled"));
    }
    this.pendingRequests.clear();
  }
}

// Export for both ES6 modules and CommonJS
export { MultiRequest };
export default MultiRequest;

// Also expose as global for direct script inclusion
if (typeof window !== "undefined") {
  window.MultiRequest = MultiRequest;
}
