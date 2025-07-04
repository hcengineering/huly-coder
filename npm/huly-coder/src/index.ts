#!/usr/bin/env node

import { spawnSync } from "child_process";

/**
 * Returns the executable path which is located inside `node_modules`
 * The naming convention is app-${os}-${arch}
 * If the platform is `win32` or `cygwin`, executable will include a `.exe` extension.
 * @see https://nodejs.org/api/os.html#osarch
 * @see https://nodejs.org/api/os.html#osplatform
 * @example "x/xx/node_modules/app-darwin-arm64"
 */
function getExePath() {
    const arch = process.arch;
    let os = process.platform as string;
    let extension = "";
    if (["win32", "cygwin"].includes(process.platform)) {
        os = "windows";
        extension = ".exe";
    }

    try {
        // Since the binary will be located inside `node_modules`, we can simply call `require.resolve`
        return require.resolve(`huly-coder-${os}-${arch}/bin/huly-coder${extension}`);
    } catch (e) {
        throw new Error(
            `Couldn't find application binary inside node_modules for ${os}-${arch}`
        );
    }
}

/**
 * Runs the application with args using nodejs spawn
 */
function run() {
    const args = process.argv.slice(2);
    const path = getExePath();
    const pathSeparator = ["win32", "cygwin"].includes(process.platform) ? "\\" : "/";
    const parentDir = path.split(pathSeparator).slice(0, -2).join(pathSeparator);
    const processResult = spawnSync(path, args, { cwd: parentDir, stdio: "inherit" });
    process.exit(processResult.status ?? 0);
}

run();