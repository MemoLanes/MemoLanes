/**
 * Platform Detection and Compatibility Module
 * Handles platform-specific checks and configurations for Android, iOS, and web
 */

import UAParser from "ua-parser-js";

// WebView compatibility check result
export interface CompatibilityCheckResult {
  compatible: boolean;
  message?: string;
  detail?: string;
}

/**
 * Check WebView version compatibility for Android and iOS using UAParser
 * Also applies iOS-specific fixes if running on iOS
 * @returns CompatibilityCheckResult indicating if the platform is compatible
 */
export function checkWebViewCompatibility(): CompatibilityCheckResult {
  // Use type assertion to handle UAParser constructor
  const parser = new (UAParser as any)(navigator.userAgent);
  const result = parser.getResult();

  const isAndroid = result.os.name === "Android";
  const isIOS = result.os.name === "iOS";

  // Check Android WebView version
  if (isAndroid) {
    // Extract Chrome/WebView version
    let chromeVersion: number | undefined;
    if (result.browser.name === "Chrome" && result.browser.version) {
      const majorVersion = result.browser.version.split(".")[0];
      chromeVersion = parseInt(majorVersion, 10);
    }

    if (!chromeVersion) {
      return { compatible: true }; // Can't determine version, allow to proceed
    }

    // https://developer.mozilla.org/en-US/docs/WebAssembly#webassembly.reference-types
    // our wasm module requires externref support in webassembly.reference-types
    // which require Android Webview 96+ or iOS 15.1+

    // Check if Chrome/WebView version is greater or equal to 96
    if (chromeVersion < 96) {
      return {
        compatible: false,
        message: "The system's WebView version is not compatible",
        detail:
          "Please update your Android System WebView to version 96 or higher.",
      };
    }

    return { compatible: true };
  }

  // Check iOS version
  if (isIOS) {
    // Extract iOS version
    let iosVersion: { major: number; minor: number } | undefined;
    if (result.os.version) {
      const versionParts = result.os.version.split(".");
      iosVersion = {
        major: parseInt(versionParts[0], 10),
        minor: parseInt(versionParts[1] || "0", 10),
      };
    }

    if (!iosVersion) {
      // Can't determine version, allow to proceed and apply iOS fixes
      preventIOSMagnifier();
      return { compatible: true };
    }

    const { major, minor } = iosVersion;

    // Check if version is greater than or equal to 15.1
    if (major < 15 || (major === 15 && minor < 1)) {
      return {
        compatible: false,
        message: "The system's iOS version is not compatible",
        detail: "Please update your iOS to version 15.1 or higher.",
      };
    }

    // Apply iOS-specific fixes for compatible iOS devices
    preventIOSMagnifier();
    return { compatible: true };
  }

  // Not Android or iOS, no check needed
  return { compatible: true };
}

/**
 * Prevent iOS magnifier/loupe on long press
 * This is iOS-specific behavior that interferes with map interaction
 */
function preventIOSMagnifier(): void {
  function createHandler(
    func: ((event: Event) => void) | null,
    timeout: number,
  ): (this: any) => void {
    let timer: number | null = null;
    let pressed: boolean = false;

    return function (this: any): void {
      // this function will be called for every touch start
      if (timer !== null) {
        clearTimeout(timer);
      }

      if (pressed) {
        if (func) {
          func.apply(this, arguments as any);
        }
        clear();
      } else {
        pressed = true;
        timer = setTimeout(clear, timeout || 500) as unknown as number;
      }
    };

    function clear(): void {
      timer = null;
      pressed = false;
    }
  }

  const ignore = createHandler((e: Event): void => {
    e.preventDefault();
  }, 500);

  // TODO: further check if applying to body is too aggressive, maybe apply to map container instead.
  document.body.addEventListener("touchstart", ignore, { passive: false });
  document.body.addEventListener(
    "touchend",
    (e: Event): void => {
      e.preventDefault();
    },
    { passive: false },
  );
}

/**
 * Display an error message for incompatible platforms
 * @param result The compatibility check result with error details
 */
export function displayCompatibilityError(
  result: CompatibilityCheckResult,
): void {
  if (!result.compatible && result.message) {
    document.body.innerHTML = `<div style="padding: 20px; font-family: Arial, sans-serif; color: red;"><h1>${result.message}</h1>${result.detail ? `<p>${result.detail}</p>` : ""}</div>`;
  }
}

/**
 * Initialize all platform-specific configurations
 * Checks compatibility and applies platform-specific fixes (e.g., iOS magnifier prevention)
 * @param onIncompatible Optional callback when platform is incompatible
 * @returns true if platform is compatible, false otherwise
 */
export function initializePlatform(
  onIncompatible?: (result: CompatibilityCheckResult) => void,
): boolean {
  // Check compatibility and apply platform-specific fixes
  const compatibilityResult = checkWebViewCompatibility();

  if (!compatibilityResult.compatible) {
    displayCompatibilityError(compatibilityResult);
    if (onIncompatible) {
      onIncompatible(compatibilityResult);
    }
    return false;
  }

  return true;
}

