/** Shared browser selection for rendering checks, previews and benchmarks. */
export function browserLaunchOptions(browserName, { headless, args = [] }) {
  if (browserName !== "chromium") return { headless };

  const executablePath = process.env.BENCHMARK_CHROMIUM_EXECUTABLE;
  const angle = process.env.BENCHMARK_CHROMIUM_ANGLE;
  const browserArgs = [...args];
  if (angle) {
    browserArgs.push(
      "--use-gl=angle",
      `--use-angle=${angle}`,
      "--enable-gpu",
      "--ignore-gpu-blocklist",
    );
    if (angle === "vulkan") browserArgs.push("--enable-features=Vulkan");
  }
  return {
    headless,
    ...(executablePath ? { executablePath } : {}),
    args: browserArgs,
  };
}
