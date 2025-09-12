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
        
        // Set up JSON response handler
        const jsonHandlerName = `handle_${this.channelName}_JsonResponse`;
        if (!window[jsonHandlerName]) {
            window[jsonHandlerName] = (responseJson) => {
                this._handleJsonResponse(responseJson);
            };
        }
    }
    
    /**
     * Handle object IPC response for this instance
     */
    _handleJsonResponse(responseData) {
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
            
            const { resolve, reject } = this.pendingRequests.get(requestId);
            
            try {
                // Return the JSON response directly
                resolve(jsonResponse);
                
            } catch (error) {
                reject(error);
            } finally {
                this.pendingRequests.delete(requestId);
            }
            
        } catch (error) {
            console.error('Failed to process response:', error);
            console.error('Raw response:', responseData);
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
        
        const response = await fetch(url, fetchOptions);
        
        if (response.ok) {
            const jsonResponse = await response.json();
            // Return the unified response directly
            return jsonResponse;
        } else {
            throw new Error(`HTTP ${response.status}: ${response.statusText}`);
        }
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
            
            // Store request info
            this.pendingRequests.set(requestId, {
                resolve,
                reject
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
