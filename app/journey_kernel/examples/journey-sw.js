import init, { JourneyBitmap } from "../pkg/journey_core.js";

let journeyBitmap;
let journeyBitmapInitialization = null;

const TILE_WIDTH = 256;

async function loadFile(filename) {
    const response = await fetch(`${filename}`);
    const arrayBuffer = await response.arrayBuffer();
    return new Uint8Array(arrayBuffer);
}

async function initializeJourneyBitmap() {
    if (journeyBitmapInitialization) {
        return journeyBitmapInitialization;
    }

    journeyBitmapInitialization = (async () => {
        await init();
        let newJourneyBitmap;

        const filename = '../journey_bitmap.bin';

        try {
            const data = await loadFile(filename);
            newJourneyBitmap = JourneyBitmap.from_bytes(data);
            console.log(`Loaded ${filename}`);
            return newJourneyBitmap;
        } catch (error) {
            console.error(`Failed to load ${filename}:`, error);
            throw error;
        }
    })();

    return journeyBitmapInitialization;
}

export async function initializeWorker() {
    return navigator.serviceWorker.register('journey-sw.js', { type: 'module' });
}

self.addEventListener('install', event => {
    event.waitUntil(self.skipWaiting());
});

self.addEventListener('activate', event => {
    event.waitUntil((async () => {
        await self.clients.claim();
        journeyBitmap = await initializeJourneyBitmap();
    })());
});

self.addEventListener('fetch', event => {
    const url = new URL(event.request.url);
    if (url.pathname.startsWith('/journey-tiles-sw/')) {
        event.respondWith(generateCustomTile(url));
    }
});

async function generateCustomTile(url) {
    if (!journeyBitmap) {
        journeyBitmap = await initializeJourneyBitmap();
    }

    const [, , z, x, y] = url.pathname.split('/');
    const time = new URL(url).searchParams.get('t');
    const canvas = new OffscreenCanvas(TILE_WIDTH, TILE_WIDTH);
    const ctx = canvas.getContext('2d');

    // Set background color with 50% opacity
    ctx.fillStyle = 'rgba(200, 200, 200, 0.5)';
    ctx.fillRect(0, 0, TILE_WIDTH, TILE_WIDTH);

    // Render the journey image
    let imageBufferRaw = await journeyBitmap.get_tile_image(x, y, z);

    // Create an ImageData object from the PNG data
    let uint8Array = new Uint8ClampedArray(imageBufferRaw);
    let imageData = new ImageData(uint8Array, TILE_WIDTH, TILE_WIDTH);

    // Draw the journey image onto the canvas
    ctx.putImageData(imageData, 0, 0);

    // Add border
    ctx.strokeStyle = 'rgba(0, 0, 0, 0.5)';
    ctx.lineWidth = 2;
    ctx.strokeRect(0, 0, TILE_WIDTH, TILE_WIDTH);

    // Write tile parameters and time
    ctx.fillStyle = 'black';
    ctx.font = '36px Arial';
    ctx.textAlign = 'center';
    ctx.fillText(`TileX: ${x}`, TILE_WIDTH / 2, 100);
    ctx.fillText(`TileY: ${y}`, TILE_WIDTH / 2, 150);
    ctx.fillText(`Zoom: ${z}`, TILE_WIDTH / 2, 200);

    // Convert canvas to blob
    const blob = await canvas.convertToBlob({ type: 'image/png' });
    return new Response(blob, {
        headers: { 'Content-Type': 'image/png' }
    });
}