"use strict";

const fs = require("fs");
const path = require("path");
const vscode = require("vscode");
const {
  LanguageClient,
  TransportKind,
} = require("vscode-languageclient/node");

let client;

function activate(context) {
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

  context.subscriptions.push(client.start());
}

function deactivate() {
  if (!client) {
    return undefined;
  }
  return client.stop();
}

function resolveServerPath(context) {
  const configured = vscode.workspace
    .getConfiguration("oxvba")
    .get("server.path", "")
    .trim();
  if (configured && fs.existsSync(configured)) {
    return configured;
  }

  const executable = process.platform === "win32" ? "oxvba-lsp.exe" : "oxvba-lsp";
  const extensionRoot = context.extensionPath;
  const repoRoot = path.resolve(extensionRoot, "..", "..");
  const candidates = [
    path.join(repoRoot, "target", "debug", executable),
    path.join(repoRoot, "target", "release", executable),
  ];
  return candidates.find((candidate) => fs.existsSync(candidate));
}

module.exports = {
  activate,
  deactivate,
};
