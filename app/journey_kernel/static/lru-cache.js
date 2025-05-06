/**
 * LRU (Least Recently Used) Cache implementation
 * Limits memory usage by removing least recently used items when capacity is reached
 */
export class LRUCache {
  constructor(capacity = 100) {
    this.capacity = capacity;
    this.cache = new Map();
    // Using Map to maintain insertion order for O(1) LRU tracking
  }

  /**
   * Get an item from the cache
   * @param {string} key - The cache key
   * @returns {*} The cached value or undefined if not found
   */
  get(key) {
    if (!this.cache.has(key)) return undefined;
    
    // Move the accessed item to the end (most recently used)
    const value = this.cache.get(key);
    this.cache.delete(key);
    this.cache.set(key, value);
    
    return value;
  }

  /**
   * Set an item in the cache
   * @param {string} key - The cache key
   * @param {*} value - The value to store
   */
  set(key, value) {
    // If key exists, remove it first to refresh its position
    if (this.cache.has(key)) {
      this.cache.delete(key);
    } 
    // If we're at capacity, remove the least recently used item (first item)
    else if (this.cache.size >= this.capacity) {
      const firstKey = this.cache.keys().next().value;
      this.cache.delete(firstKey);
    }
    
    // Add the new item (will be at the end - most recently used)
    this.cache.set(key, value);
  }

  /**
   * Check if key exists in cache
   * @param {string} key - The cache key
   * @returns {boolean} True if key exists
   */
  has(key) {
    return this.cache.has(key);
  }

  /**
   * Clear the cache
   */
  clear() {
    this.cache.clear();
  }

  /**
   * Get current cache size
   * @returns {number} Number of items in cache
   */
  get size() {
    return this.cache.size;
  }
} 