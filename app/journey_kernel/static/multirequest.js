/**
 * MultiRequest Module
 * 
 * A unified JavaScript class for handling both HTTP and Flutter IPC communications 
 * with a consistent API.
 */

/**
 * MultiRequest Class
 * 
 * Provides a unified interface for making both HTTP requests and Flutter IPC calls.
 * Automatically routes requests based on the endpoint protocol (http:// vs flutter://).
 */
class MultiRequest {
    constructor(cgiEndpoint = null) {
        this.cgiEndpoint = cgiEndpoint;
        this.requestId = 0;
        this.pendingRequests = new Map();
        this.channelName = null;
        this.channel = null;
        this.timeout = 30000; // Default 30 second timeout
        
        // Parse flutter:// endpoints
        if (cgiEndpoint && cgiEndpoint.startsWith('flutter://')) {
            this.channelName = cgiEndpoint.replace('flutter://', '');
            this.channel = window[this.channelName];
            
            // Set up response handler for this specific instance
            this._setupResponseHandler();
        }
    }
    
    /**
     * Set up response handler for Flutter IPC for this specific instance
     */
    _setupResponseHandler() {
        if (!this.channelName) return;
        
        // Create a unique handler name for this instance (legacy format)
        const handlerName = `handle_${this.channelName}_Response`;
        
        // Set up legacy handler if it doesn't exist
        if (!window[handlerName]) {
            window[handlerName] = (requestId, base64Data, size, processingTime) => {
                this._handleResponse(requestId, base64Data, size, processingTime);
            };
        }
        
        // Set up new JSON response handler
        const jsonHandlerName = `handle_${this.channelName}_JsonResponse`;
        if (!window[jsonHandlerName]) {
            window[jsonHandlerName] = (responseJson) => {
                this._handleJsonResponse(responseJson);
            };
        }
        
        // Also support the generic handlers for backwards compatibility
        if (!window.handleIpcResponse) {
            window.handleIpcResponse = (requestId, base64Data, size, processingTime) => {
                this._handleResponse(requestId, base64Data, size, processingTime);
            };
        }
    }
    
    /**
     * Handle raw JSON IPC response for this instance
     */
    _handleJsonResponse(responseJson) {
        try {
            const jsonResponse = JSON.parse(responseJson);
            const requestId = parseInt(jsonResponse.requestId);
            
            if (!this.pendingRequests.has(requestId)) {
                // This response might be for a different instance
                return;
            }
            
            const { resolve, reject, startTime } = this.pendingRequests.get(requestId);
            
            try {
                const endTime = performance.now();
                // Use the same unified response creation method as HTTP
                const unifiedResponse = this._createUnifiedResponse(jsonResponse, startTime, endTime);
                resolve(unifiedResponse);
                
            } catch (error) {
                reject(error);
            } finally {
                this.pendingRequests.delete(requestId);
            }
            
        } catch (error) {
            console.error('Failed to parse JSON response:', error);
            console.error('Raw response:', responseJson);
        }
    }

    /**
     * Handle IPC response for this instance (legacy format)
     */
    _handleResponse(requestId, base64Data, size, processingTime) {
        const numericRequestId = parseInt(requestId);
        
        if (!this.pendingRequests.has(numericRequestId)) {
            // This response might be for a different instance
            return;
        }
        
        const { resolve, startTime } = this.pendingRequests.get(numericRequestId);
        
        try {
            const endTime = performance.now();
            const totalTime = Math.round(endTime - startTime);
            const flutterTime = Math.round(processingTime / 1000); // Convert to ms
            
            // Efficiently decode base64 data
            const binaryString = atob(base64Data);
            const bytes = new Uint8Array(binaryString.length);
            for (let i = 0; i < binaryString.length; i++) {
                bytes[i] = binaryString.charCodeAt(i);
            }
            
            // Create response object similar to fetch Response
            const response = {
                ok: true,
                status: 200,
                data: bytes,
                size: size,
                totalTime: totalTime,
                processingTime: flutterTime,
                transferTime: totalTime - flutterTime,
                requestId: numericRequestId,
                
                // Add methods to make it more fetch-like
                arrayBuffer: async () => bytes.buffer,
                blob: async () => new Blob([bytes]),
                json: async () => {
                    const text = new TextDecoder().decode(bytes);
                    return JSON.parse(text);
                },
                text: async () => new TextDecoder().decode(bytes)
            };
            
            resolve(response);
            
        } catch (error) {
            const { reject } = this.pendingRequests.get(numericRequestId);
            reject(new Error(`Failed to process IPC response: ${error.message}`));
        } finally {
            this.pendingRequests.delete(numericRequestId);
        }
    }
    
    /**
     * Set the CGI endpoint for this instance
     */
    setEndpoint(cgiEndpoint) {
        this.cgiEndpoint = cgiEndpoint;
        
        // Re-parse if it's a flutter endpoint
        if (cgiEndpoint && cgiEndpoint.startsWith('flutter://')) {
            this.channelName = cgiEndpoint.replace('flutter://', '');
            this.channel = window[this.channelName];
            this._setupResponseHandler();
        } else {
            this.channelName = null;
            this.channel = null;
        }
    }
    
    /**
     * Set timeout for requests
     */
    setTimeout(timeout) {
        this.timeout = timeout;
    }

    /**
     * Main fetch method that routes to HTTP or Flutter IPC
     */
    async fetch(resource, params = null, options = {}) {
        if (!this.cgiEndpoint) {
            throw new Error('No CGI endpoint set. Call setEndpoint() first.');
        }
        
        if (this.cgiEndpoint.startsWith('flutter://')) {
            return this.fetchViaFlutter(resource, params, options);
        } else {
            return this.fetchViaHttp(resource, params, options);
        }
    }
    
    /**
     * HTTP fetch implementation - Always uses unified JSON API
     */
    async fetchViaHttp(resource, params = null, options = {}) {
        const url = `${this.cgiEndpoint}/api`;
        
        // Generate unique request ID
        const requestId = `http_${this.requestId++}_${Date.now()}`;
        
        // Create unified request payload matching Rust Request struct
        const unifiedRequest = {
            requestId: requestId,
            query: resource,
            payload: params || {}
        };
        
        const fetchOptions = {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
                ...(options.headers || {})
            },
            body: JSON.stringify(unifiedRequest),
            cache: 'no-cache',
            ...options
        };
        
        const startTime = performance.now();
        const response = await fetch(url, fetchOptions);
        const endTime = performance.now();
        
        if (response.ok) {
            const jsonResponse = await response.json();
            // Process the unified response format and add timing
            return this._createUnifiedResponse(jsonResponse, startTime, endTime);
        } else {
            throw new Error(`HTTP ${response.status}: ${response.statusText}`);
        }
    }
    
    /**
     * Create unified response object from Rust JSON response
     */
    _createUnifiedResponse(jsonResponse, startTime, endTime) {
        const totalTime = Math.round(endTime - startTime);
        
        if (!jsonResponse.success) {
            throw new Error(jsonResponse.error || 'Request failed');
        }
        
        // Create a fetch-like response object with the unified data
        return {
            ok: true,
            status: 200,
            requestId: jsonResponse.requestId,
            success: jsonResponse.success,
            data: jsonResponse.data,
            totalTime: totalTime,
            
            // Standard fetch-like methods that work with the unified response format
            json: async () => jsonResponse.data,
            text: async () => JSON.stringify(jsonResponse.data),
            arrayBuffer: async () => {
                // If data contains base64 encoded binary data (like tile_range), decode it
                if (jsonResponse.data && jsonResponse.data.body) {
                    const binaryString = atob(jsonResponse.data.body);
                    const bytes = new Uint8Array(binaryString.length);
                    for (let i = 0; i < binaryString.length; i++) {
                        bytes[i] = binaryString.charCodeAt(i);
                    }
                    return bytes.buffer;
                }
                return new ArrayBuffer(0);
            },
            blob: async function() {
                const arrayBuffer = await this.arrayBuffer();
                return new Blob([arrayBuffer]);
            }
        };
    }

    /**
     * Flutter IPC fetch implementation
     */
    async fetchViaFlutter(resource, params = null, options = {}) {
        if (!this.channel) {
            throw new Error(`Flutter channel '${this.channelName}' not available`);
        }
        
        return new Promise((resolve, reject) => {
            const requestId = ++this.requestId;
            const startTime = performance.now();
            
            // Store request info
            this.pendingRequests.set(requestId, {
                resolve,
                reject,
                startTime
            });
            
            // Send unified request format via JavaScript channel
            try {
                const unifiedRequest = {
                    requestId: requestId.toString(),
                    query: resource,
                    payload: params || {}
                };
                
                this.channel.postMessage(JSON.stringify(unifiedRequest));
                
            } catch (error) {
                this.pendingRequests.delete(requestId);
                reject(new Error(`Failed to send Flutter IPC request: ${error.message}`));
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
    getStatus() {
        return {
            endpoint: this.cgiEndpoint,
            channelName: this.channelName,
            channelAvailable: !!this.channel,
            pendingRequests: this.pendingRequests.size,
            isFlutterMode: this.cgiEndpoint && this.cgiEndpoint.startsWith('flutter://'),
            isHttpMode: this.cgiEndpoint && !this.cgiEndpoint.startsWith('flutter://')
        };
    }
    
    /**
     * Clear all pending requests (useful for cleanup)
     */
    clearPending() {
        for (const [requestId, { reject }] of this.pendingRequests) {
            reject(new Error('Request cancelled'));
        }
        this.pendingRequests.clear();
    }
}

// Export for both ES6 modules and CommonJS
export { MultiRequest };
export default MultiRequest;

// Also expose as global for direct script inclusion
if (typeof window !== 'undefined') {
    window.MultiRequest = MultiRequest;
}
