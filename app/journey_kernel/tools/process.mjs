import { spawn } from "node:child_process";

export function startProcess(command, args, cwd, extraEnvironment = {}) {
  const child = spawn(command, args, {
    cwd,
    env: { ...process.env, ...extraEnvironment },
    detached: process.platform !== "win32",
    stdio: ["ignore", "pipe", "pipe"],
  });
  child.output = "";
  child.spawnError = null;
  const collect = (chunk) => {
    child.output = `${child.output}${chunk}`.slice(-40_000);
  };
  child.stdout.setEncoding("utf8");
  child.stderr.setEncoding("utf8");
  child.stdout.on("data", collect);
  child.stderr.on("data", collect);
  child.on("error", (error) => {
    child.spawnError = error;
    collect(`${error.stack ?? error}\n`);
  });
  return child;
}

export async function waitForHttp(url, child, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (hasProcessStopped(child)) {
      throw new Error(`Web bundle server exited early:\n${child.output}`);
    }
    try {
      const response = await fetch(url);
      if (response.ok) {
        return;
      }
    } catch {
      // Compilation or server startup is still in progress.
    }
    await delay(250);
  }
  throw new Error(`Timed out waiting for ${url}:\n${child.output}`);
}

export function waitForOutput(child, pattern, timeoutMs) {
  return new Promise((resolve, reject) => {
    const inspect = () => {
      const match = child.output.match(pattern);
      if (match) {
        finish();
        resolve(match[1]);
        return;
      }
      if (hasProcessStopped(child)) {
        finish();
        reject(new Error(`Child process exited early:\n${child.output}`));
      }
    };
    const timeout = setTimeout(() => {
      finish();
      reject(
        new Error(`Timed out waiting for process output:\n${child.output}`),
      );
    }, timeoutMs);
    const finish = () => {
      clearTimeout(timeout);
      child.stdout.off("data", inspect);
      child.stderr.off("data", inspect);
      child.off("error", inspect);
      child.off("exit", inspect);
    };

    child.stdout.on("data", inspect);
    child.stderr.on("data", inspect);
    child.once("error", inspect);
    child.once("exit", inspect);
    inspect();
  });
}

export async function stopProcess(child) {
  if (hasProcessStopped(child)) {
    return;
  }
  signalProcess(child, "SIGINT");
  if (await waitForExit(child, 2_000)) {
    return;
  }
  signalProcess(child, "SIGTERM");
  if (await waitForExit(child, 2_000)) {
    return;
  }
  signalProcess(child, "SIGKILL");
}

function signalProcess(child, signal) {
  try {
    if (process.platform !== "win32" && child.pid) {
      process.kill(-child.pid, signal);
    } else {
      child.kill(signal);
    }
  } catch {
    // The process may have exited between the state check and the signal.
  }
}

function waitForExit(child, timeoutMs) {
  if (hasProcessStopped(child)) {
    return Promise.resolve(true);
  }
  return new Promise((resolve) => {
    const timeout = setTimeout(() => {
      child.off("exit", onExit);
      resolve(false);
    }, timeoutMs);
    const onExit = () => {
      clearTimeout(timeout);
      resolve(true);
    };
    child.once("exit", onExit);
  });
}

function hasProcessStopped(child) {
  return (
    child.spawnError !== null ||
    child.exitCode !== null ||
    child.signalCode !== null
  );
}

function delay(durationMs) {
  return new Promise((resolve) => setTimeout(resolve, durationMs));
}
