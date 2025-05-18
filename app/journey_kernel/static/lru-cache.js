/**
 * LRU (Least Recently Used) Cache implementation
 * Limits memory usage by removing least recently used items when capacity is reached
 */
export class LRUCache {
  constructor(capacity = 100) {
    this.capacity = capacity;
    this.cache = new Map();
    this.keyOrder = [];
    // Using Map to maintain insertion order for O(1) LRU tracking
  }

  /**
   * Get an item from the cache
   * @param {string} key - The cache key
   * @returns {*} The cached value or undefined if not found
   */
  get(key) {
    if (!this.cache.has(key)) return null;
    
    // Move the accessed key to the end of the order array (most recently used)
    this.keyOrder = this.keyOrder.filter(k => k !== key);
    this.keyOrder.push(key);
    
    return this.cache.get(key);
  }

  /**
   * Set an item in the cache
   * @param {string} key - The cache key
   * @param {*} value - The value to store
   */
  set(key, value) {
    // If already exists, update order
    if (this.cache.has(key)) {
      this.keyOrder = this.keyOrder.filter(k => k !== key);
    } 
    // If at capacity, remove least recently used item
    else if (this.keyOrder.length >= this.capacity) {
      const lruKey = this.keyOrder.shift();
      this.cache.delete(lruKey);
    }
    
    // Add new item
    this.cache.set(key, value);
    this.keyOrder.push(key);
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
    this.keyOrder = [];
  }

  /**
   * Get current cache size
   * @returns {number} Number of items in cache
   */
  get size() {
    return this.cache.size;
  }

  // Remove an item from the cache
  remove(key) {
    if (this.cache.has(key)) {
      this.cache.delete(key);
      this.keyOrder = this.keyOrder.filter(k => k !== key);
      return true;
    }
    return false;
  }
} 