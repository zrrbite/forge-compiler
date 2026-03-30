const { LanguageClient, TransportKind } = require("vscode-languageclient/node");
const vscode = require("vscode");

let client;

function activate(context) {
  const config = vscode.workspace.getConfiguration("forge");
  const lspPath = config.get("lspPath", "forge-lsp");

  const serverOptions = {
    command: lspPath,
    transport: TransportKind.stdio,
  };

  const clientOptions = {
    documentSelector: [{ scheme: "file", language: "forge" }],
  };

  client = new LanguageClient(
    "forge-lsp",
    "Forge Language Server",
    serverOptions,
    clientOptions
  );

  client.start();
}

function deactivate() {
  if (client) {
    return client.stop();
  }
}

module.exports = { activate, deactivate };
