/**
 * Platform Detection and Compatibility Module
 * Handles platform-specific checks and configurations for Android, iOS, and web
 */

// Platform detection results
export interface PlatformInfo {
  isAndroid: boolean;
  isIOS: boolean;
  isWeb: boolean;
  userAgent: string;
  androidVersion?: number;
  iosVersion?: { major: number; minor: number; patch?: number };
  chromeVersion?: number;
}

// WebView compatibility check result
export interface CompatibilityCheckResult {
  compatible: boolean;
  message?: string;
  detail?: string;
}

/**
 * Detect current platform and extract version information
 * @returns PlatformInfo object with platform details
 */
export function detectPlatform(): PlatformInfo {
  const ua = navigator.userAgent;
  const isAndroid = /Android/i.test(ua);
  const isIOS = /iPhone|iPad|iPod/i.test(ua);

  const platformInfo: PlatformInfo = {
    isAndroid,
    isIOS,
    isWeb: !isAndroid && !isIOS,
    userAgent: ua,
  };

  // Extract Android version
  if (isAndroid) {
    const androidMatch = ua.match(/Android\s+([\d.]+)/i);
    if (androidMatch) {
      platformInfo.androidVersion = parseFloat(androidMatch[1]);
    }

    // Extract Chrome/WebView version
    const chromeMatch = ua.match(/Chrome\/(\d+)\.(\d+)\.(\d+)/);
    if (chromeMatch) {
      platformInfo.chromeVersion = parseInt(chromeMatch[1], 10);
    }
  }

  // Extract iOS version
  if (isIOS) {
    const iosMatch = ua.match(/OS (\d+)_(\d+)(?:_(\d+))?/);
    if (iosMatch) {
      platformInfo.iosVersion = {
        major: parseInt(iosMatch[1], 10),
        minor: parseInt(iosMatch[2], 10),
        patch: iosMatch[3] ? parseInt(iosMatch[3], 10) : undefined,
      };
    }
  }

  return platformInfo;
}

/**
 * Check WebView version compatibility for Android and iOS
 * @returns CompatibilityCheckResult indicating if the platform is compatible
 */
export function checkWebViewCompatibility(): CompatibilityCheckResult {
  const platform = detectPlatform();

  // Check Android WebView version
  if (platform.isAndroid) {
    if (!platform.chromeVersion) {
      return { compatible: true }; // Can't determine version, allow to proceed
    }

    // Check if version is greater or equal to 96
    if (platform.chromeVersion <= 95) {
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
  if (platform.isIOS) {
    if (!platform.iosVersion) {
      return { compatible: true }; // Can't determine version, allow to proceed
    }

    const { major, minor } = platform.iosVersion;

    // Check if version is greater than or equal to 15.1
    if (major < 15 || (major === 15 && minor < 1)) {
      return {
        compatible: false,
        message: "The system's iOS version is not compatible",
        detail: "Please update your iOS to version 15.1 or higher.",
      };
    }

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
 * Apply iOS-specific fixes if running on iOS
 */
export function applyIOSFixes(): void {
  const platform = detectPlatform();
  if (platform.isIOS) {
    preventIOSMagnifier();
  }
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
 * @param onIncompatible Optional callback when platform is incompatible
 * @returns true if platform is compatible, false otherwise
 */
export function initializePlatform(
  onIncompatible?: (result: CompatibilityCheckResult) => void,
): boolean {
  // Check compatibility first
  const compatibilityResult = checkWebViewCompatibility();

  if (!compatibilityResult.compatible) {
    displayCompatibilityError(compatibilityResult);
    if (onIncompatible) {
      onIncompatible(compatibilityResult);
    }
    return false;
  }

  // Apply platform-specific fixes
  applyIOSFixes();

  return true;
}

/**
 * Get a human-readable platform description for logging
 * @returns Platform description string
 */
export function getPlatformDescription(): string {
  const platform = detectPlatform();

  if (platform.isAndroid) {
    const version = platform.androidVersion
      ? ` ${platform.androidVersion}`
      : "";
    const chrome = platform.chromeVersion
      ? ` (Chrome ${platform.chromeVersion})`
      : "";
    return `Android${version}${chrome}`;
  }

  if (platform.isIOS) {
    const version = platform.iosVersion
      ? ` ${platform.iosVersion.major}.${platform.iosVersion.minor}${platform.iosVersion.patch ? `.${platform.iosVersion.patch}` : ""}`
      : "";
    return `iOS${version}`;
  }

  return "Web Browser";
}
