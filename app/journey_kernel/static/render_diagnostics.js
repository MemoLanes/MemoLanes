// === COORDINATION SETUP ===
window.JS_READY = false;
window.EXTERNAL_PARAMS = null;

let isRunning = false;
let intervalId = null;
let requestCounter = 0;
let endpointCounter = 0;
let apiEndpoint = null;

// === IPC CHANNEL SETUP ===
let ipcRequestId = 0;
const pendingIpcRequests = new Map();

function initializeTest() {
    if (window.JS_READY && window.EXTERNAL_PARAMS) {
        console.log("Initializing test with:", window.EXTERNAL_PARAMS);
        
        // Set the API endpoint
        apiEndpoint = window.EXTERNAL_PARAMS.api_endpoint;
        
        // Update UI
        const statusDiv = document.getElementById('status');
        statusDiv.className = 'status ready';
        statusDiv.textContent = `Ready! HTTP: ${apiEndpoint}, IPC: Flutter Channel`;
        
        // Enable start button
        document.getElementById('startBtn').disabled = false;
        
        log(`Test ready - HTTP endpoint: ${apiEndpoint}`);
        log(`Test ready - IPC channel: ${typeof window.RenderDiagnosticsChannel !== 'undefined' ? 'Available' : 'Not Available'}`);
    }
}

function log(message) {
    const logDiv = document.getElementById('log');
    const timestamp = new Date().toLocaleTimeString();
    logDiv.innerHTML += `[${timestamp}] ${message}<br>`;
    logDiv.scrollTop = logDiv.scrollHeight;
}

// === IPC CHANNEL FUNCTIONS ===
async function makeIpcRequest(size) {
    return new Promise((resolve, reject) => {
        const requestId = ++ipcRequestId;
        const startTime = performance.now();
        
        pendingIpcRequests.set(requestId, { resolve, reject, startTime });
        
        // Send request via JavaScript channel (most efficient)
        if (window.RenderDiagnosticsChannel) {
            window.RenderDiagnosticsChannel.postMessage(JSON.stringify({
                requestId: requestId,
                size: size
            }));
        } else {
            reject(new Error('IPC Channel not available'));
            return;
        }
        
        // Timeout after 30 seconds
        setTimeout(() => {
            if (pendingIpcRequests.has(requestId)) {
                pendingIpcRequests.delete(requestId);
                reject(new Error('IPC request timeout'));
            }
        }, 30000);
    });
}

// Handle IPC responses from Flutter (called via runJavaScript)
window.handleIpcResponse = function(requestId, base64Data, size, processingTime) {
    if (pendingIpcRequests.has(requestId)) {
        const { resolve, startTime } = pendingIpcRequests.get(requestId);
        
        const endTime = performance.now();
        const totalTime = Math.round(endTime - startTime);
        const flutterTime = Math.round(processingTime / 1000); // Convert to ms
        
        // Efficiently decode base64 data
        const binaryString = atob(base64Data);
        const bytes = new Uint8Array(binaryString.length);
        for (let i = 0; i < binaryString.length; i++) {
            bytes[i] = binaryString.charCodeAt(i);
        }
        
        resolve({
            data: bytes,
            size: size,
            totalTime: totalTime,
            processingTime: flutterTime,
            transferTime: totalTime - flutterTime
        });
        
        pendingIpcRequests.delete(requestId);
    }
};

// Handle IPC errors
window.handleIpcError = function(error) {
    log(`IPC ERROR: ${error}`);
};

// === REQUEST FUNCTIONS ===
// Simplify the getCurrentEndpoint function since we only have two types now
function getCurrentEndpoint() {
    const sizeSelect = document.getElementById('sizeSelect');
    const size = sizeSelect ? sizeSelect.value : '1048576';
    
    const groupIndex = Math.floor(endpointCounter / 5) % 2;
    return {
        path: groupIndex === 0 ? 'download1M' : 'ipc',
        size: parseInt(size),
        isIpc: groupIndex === 1 // Every other 5 requests use IPC
    };
}

// Update the makeRequest function
async function makeRequest() {
    if (!apiEndpoint) {
        log('ERROR: No API endpoint set!');
        return;
    }

    const requestId = ++requestCounter;
    const { path, size, isIpc } = getCurrentEndpoint();
    endpointCounter++;
    
    const startTime = performance.now();
    
    if (isIpc) {
        log(`Request #${requestId}: IPC Channel - ${size} bytes - Starting...`);
        
        try {
            const response = await makeIpcRequest(size);
            log(`Request #${requestId}: IPC Channel - SUCCESS - ${response.totalTime}ms total (${response.processingTime}ms Flutter + ${response.transferTime}ms transfer) - ${response.size} bytes`);
        } catch (error) {
            const endTime = performance.now();
            const duration = Math.round(endTime - startTime);
            log(`Request #${requestId}: IPC Channel - ERROR - ${error.message} - ${duration}ms`);
        }
    } else {
        // HTTP request to the single endpoint
        const endpoint = `${apiEndpoint}/${path}?size=${size}`;
        log(`Request #${requestId}: HTTP (${path}) - ${size} bytes - Starting...`);

        try {
            const response = await fetch(endpoint, {
                method: 'GET',
                cache: 'no-cache'
            });
            
            const endTime = performance.now();
            const duration = Math.round(endTime - startTime);
            
            if (response.ok) {
                log(`Request #${requestId}: HTTP (${path}) - SUCCESS - ${duration}ms`);
            } else {
                log(`Request #${requestId}: HTTP (${path}) - ERROR - HTTP ${response.status} - ${duration}ms`);
            }
            
        } catch (error) {
            const endTime = performance.now();
            const duration = Math.round(endTime - startTime);
            log(`Request #${requestId}: HTTP (${path}) - ERROR - ${error.message} - ${duration}ms`);
        }
    }
}

function startTest() {
    if (isRunning || !apiEndpoint) return;
    
    isRunning = true;
    document.getElementById('startBtn').disabled = true;
    document.getElementById('stopBtn').disabled = false;
    
    log('Test started - alternating HTTP (download1M) and IPC (Flutter API) every 5 requests');
    
    makeRequest();
    intervalId = setInterval(makeRequest, 1000);
}

function stopTest() {
    if (!isRunning) return;
    
    isRunning = false;
    document.getElementById('startBtn').disabled = false;
    document.getElementById('stopBtn').disabled = true;
    
    if (intervalId) {
        clearInterval(intervalId);
        intervalId = null;
    }
    log('Test stopped');
}

function clearLog() {
    document.getElementById('log').innerHTML = '';
    requestCounter = 0;
    endpointCounter = 0;
    if (apiEndpoint) {
        log(`Log cleared - HTTP: ${apiEndpoint}, IPC: Available`);
    }
}

// Expose functions to window object to prevent webpack from renaming them
window.initializeTest = initializeTest;
window.startTest = startTest;
window.stopTest = stopTest;
window.clearLog = clearLog;

// Initialize when DOM is ready
document.addEventListener('DOMContentLoaded', function() {
    // Mark JS as ready
    window.JS_READY = true;
    initializeTest(); // Try to initialize
    
    window.addEventListener('beforeunload', stopTest);
    log('Page loaded - waiting for API endpoint...');
});
