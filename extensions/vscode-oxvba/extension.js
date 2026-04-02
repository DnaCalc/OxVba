"use strict";

const cp = require("child_process");
const fs = require("fs");
const path = require("path");
const vscode = require("vscode");
const {
  LanguageClient,
  TransportKind,
} = require("vscode-languageclient/node");

let client;
let outputChannel;

function activate(context) {
  outputChannel = vscode.window.createOutputChannel("OxVba");
  const serverPath = resolveServerPath(context);
  if (!serverPath) {
    vscode.window.showErrorMessage(
      "OxVba: could not find oxvba-lsp. Set oxvba.server.path or build the repo-local binary first."
    );
    return;
  }

  const serverOptions = {
    run: {
      command: serverPath,
      transport: TransportKind.stdio,
    },
    debug: {
      command: serverPath,
      transport: TransportKind.stdio,
    },
  };

  const clientOptions = {
    documentSelector: [
      { scheme: "file", language: "oxvba" },
    ],
    synchronize: {
      fileEvents: vscode.workspace.createFileSystemWatcher("**/*.{bas,cls,frm,basproj,vbp}"),
    },
  };

  client = new LanguageClient(
    "oxvba-lsp",
    "OxVba Language Server",
    serverOptions,
    clientOptions
  );

  context.subscriptions.push(
    outputChannel,
    client.start(),
    vscode.commands.registerCommand("oxvba.initProject", () => initProject(context)),
    vscode.commands.registerCommand("oxvba.captureConventionProject", () =>
      captureConventionProject(context)
    ),
    vscode.commands.registerCommand("oxvba.addComReference", () =>
      addComReference(context)
    ),
    vscode.commands.registerCommand("oxvba.repairComReferences", () =>
      repairComReferences(context)
    )
  );
}

function deactivate() {
  if (!client) {
    return undefined;
  }
  return client.stop();
}

function resolveServerPath(context) {
  return resolveBinaryPath(context, "server.path", "oxvba-lsp");
}

function resolveCliPath(context) {
  return resolveBinaryPath(context, "cli.path", "oxvba-cli");
}

function resolveBinaryPath(context, settingName, baseName) {
  const configured = vscode.workspace
    .getConfiguration("oxvba")
    .get(settingName, "")
    .trim();
  if (configured && fs.existsSync(configured)) {
    return configured;
  }

  const executable = process.platform === "win32" ? `${baseName}.exe` : baseName;
  const extensionRoot = context.extensionPath;
  const repoRoot = path.resolve(extensionRoot, "..", "..");
  const candidates = [
    path.join(repoRoot, "target", "debug", executable),
    path.join(repoRoot, "target", "release", executable),
  ];
  return candidates.find((candidate) => fs.existsSync(candidate));
}

async function initProject(context) {
  const target = await pickFolder("Select a folder for the new OxVba project");
  if (!target) {
    return;
  }

  const kind = await vscode.window.showQuickPick(
    [
      { label: "Application", value: "application" },
      { label: "Library", value: "library" },
      { label: "Add-in", value: "addin" },
      { label: "Host Module", value: "host-module" },
      { label: "COM Server", value: "com-server" },
      { label: "COM EXE", value: "com-exe" },
    ],
    { title: "OxVba project kind" }
  );
  if (!kind) {
    return;
  }

  await runOxvbaCli(context, ["init", target.fsPath, "--kind", kind.value], {
    successMessage: `Initialized OxVba ${kind.value} project in ${target.fsPath}`,
  });
}

async function captureConventionProject(context) {
  const target = await pickFolder("Select a convention folder to capture into .basproj");
  if (!target) {
    return;
  }

  await runOxvbaCli(context, ["init", target.fsPath, "--from-convention"], {
    successMessage: `Captured convention folder into ${target.fsPath}`,
  });
}

async function addComReference(context) {
  const workspaceFolder = await pickWorkspaceFolder();
  if (!workspaceFolder) {
    return;
  }

  const mode = await vscode.window.showQuickPick(
    [
      { label: "Library name", value: "name" },
      { label: "ProgID", value: "progid" },
      { label: "Type library file", value: "file" },
    ],
    { title: "How should OxVba locate the COM reference?" }
  );
  if (!mode) {
    return;
  }

  const args = ["com-ref", "add", workspaceFolder.uri.fsPath];
  if (mode.value === "file") {
    const picked = await vscode.window.showOpenDialog({
      title: "Select a COM type library carrier",
      canSelectFiles: true,
      canSelectFolders: false,
      canSelectMany: false,
      filters: {
        "Type Library Carriers": ["tlb", "olb", "dll", "ocx", "exe", "xll"],
      },
    });
    if (!picked || picked.length === 0) {
      return;
    }
    args.push("--file", picked[0].fsPath);
  } else {
    const value = await vscode.window.showInputBox({
      title: mode.value === "name" ? "COM library name" : "COM ProgID",
      prompt:
        mode.value === "name"
          ? "Enter a registered library name such as Scripting or Excel"
          : "Enter a ProgID such as Scripting.FileSystemObject",
      ignoreFocusOut: true,
    });
    if (!value) {
      return;
    }
    args.push(`--${mode.value}`, value);
  }

  await runOxvbaCli(context, args, {
    successMessage: `Updated COM references for ${workspaceFolder.name}`,
  });
}

async function repairComReferences(context) {
  const workspaceFolder = await pickWorkspaceFolder();
  if (!workspaceFolder) {
    return;
  }

  await runOxvbaCli(context, ["com-ref", "repair", workspaceFolder.uri.fsPath], {
    successMessage: `Repaired COM references for ${workspaceFolder.name}`,
  });
}

async function pickFolder(title) {
  const picked = await vscode.window.showOpenDialog({
    title,
    canSelectFiles: false,
    canSelectFolders: true,
    canSelectMany: false,
  });
  return picked && picked.length > 0 ? picked[0] : undefined;
}

async function pickWorkspaceFolder() {
  const folders = vscode.workspace.workspaceFolders || [];
  if (folders.length === 0) {
    vscode.window.showErrorMessage("OxVba: open a workspace folder first.");
    return undefined;
  }
  if (folders.length === 1) {
    return folders[0];
  }
  return vscode.window.showWorkspaceFolderPick({
    title: "Select the OxVba workspace folder",
  });
}

async function runOxvbaCli(context, args, options) {
  const cliPath = resolveCliPath(context);
  if (!cliPath) {
    vscode.window.showErrorMessage(
      "OxVba: could not find oxvba-cli. Set oxvba.cli.path or build the repo-local binary first."
    );
    return;
  }

  outputChannel.appendLine(`> ${cliPath} ${args.join(" ")}`);
  outputChannel.show(true);

  const cwd = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath || context.extensionPath;
  const result = await new Promise((resolve) => {
    cp.execFile(cliPath, args, { cwd }, (error, stdout, stderr) => {
      resolve({ error, stdout, stderr });
    });
  });

  if (result.stdout) {
    outputChannel.append(result.stdout);
    if (!result.stdout.endsWith("\n")) {
      outputChannel.appendLine("");
    }
  }
  if (result.stderr) {
    outputChannel.append(result.stderr);
    if (!result.stderr.endsWith("\n")) {
      outputChannel.appendLine("");
    }
  }

  if (result.error) {
    vscode.window.showErrorMessage(`OxVba command failed: ${result.error.message}`);
    return;
  }

  if (options && options.successMessage) {
    vscode.window.showInformationMessage(options.successMessage);
  }
}

module.exports = {
  activate,
  deactivate,
};
