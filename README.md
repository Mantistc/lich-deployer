# Solana Program Deployer GUI
 
 A **Solana GUI Executable application** for deploying programs, built entirely in **Rust**. This application provides an intuitive interface to deploy Solana programs.
 
 ---
 
 ## **Features**
 - Integrated keypair management.
 - Configurable options for:
   - Unit limits.
   - Priority fees.
 - Full support for Solana's latest deployment workflow.
 
 ---
 
 ## **Getting Started**
 
 ### **Requirements**
 1. **Rust** (latest stable version).
 2. **Solana CLI** installed and configured.
 3. A valid **keypair** file for signing transactions.
 4. A valid builded program file `.so`
 
 ### **Installation**
 1. Clone this repository:
    ```bash
    git clone https://github.com/Mantistc/lich-deployer
    cd lich-deployer
    ```
 2. Build the application:
    ```bash
    cargo build --release
    ```
 3. Run the application:
    ```bash
    cargo run --release
    ```
 ---
 
 ## **Built With**
 - **Iced**: Cool GUI library for Rust.
 
 ---
 
 <p align="center">
   Made with ❤️ by <a href="https://twitter.com/lich01_" target="_blank">@lich.sol</a>
 </p>
