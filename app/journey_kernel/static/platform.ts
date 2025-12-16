/**
 * Platform Detection and Compatibility Module
 * Handles platform-specific checks and configurations for Android, iOS, and web
 */

import UAParser from "ua-parser-js";

/**
 * Ensure platform compatibility by checking WebView versions and applying platform-specific fixes
 * - For Android: Checks WebView version (requires v96+)
 * - For iOS: Checks iOS version (requires v15.1+) and applies workarounds (prevents magnifier)
 * @throws Error if the platform is not compatible
 */
export function ensurePlatformCompatibility(): void {
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
      return; // Can't determine version, allow to proceed
    }

    // https://developer.mozilla.org/en-US/docs/WebAssembly#webassembly.reference-types
    // our wasm module requires externref support in webassembly.reference-types
    // which require Android Webview 96+ or iOS 15.1+

    // Check if Chrome/WebView version is greater or equal to 96
    if (chromeVersion < 96) {
      throw new Error(
        "The system's WebView version is not compatible. Please update your Android System WebView to version 96 or higher.",
      );
    }

    return;
  }

  // Check iOS version
  if (isIOS) {
    // Apply iOS-specific fixes first (needed for all iOS versions)
    preventIOSMagnifier();

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
      // Can't determine version, allow to proceed (iOS fix already applied)
      return;
    }

    const { major, minor } = iosVersion;

    // Check if version is greater than or equal to 15.1
    if (major < 15 || (major === 15 && minor < 1)) {
      throw new Error(
        "The system's iOS version is not compatible. Please update your iOS to version 15.1 or higher.",
      );
    }

    return;
  }

  // Not Android or iOS, no check needed
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
