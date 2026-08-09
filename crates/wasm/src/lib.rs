use std::collections::HashMap;
use thiserror::Error;
use wasmtime::{Config, Engine, Linker, Module, Store};

#[derive(Debug, Error, PartialEq, Eq)]
pub enum WasmError {
    #[error("compile error: {0}")]
    Compile(String),
    #[error("instantiate error: {0}")]
    Instantiate(String),
    #[error("call error: {0}")]
    Call(String),
    #[error("fuel exhausted")]
    FuelExhausted,
    #[error("export not found: {0}")]
    ExportNotFound(String),
    #[error("unknown contract")]
    UnknownContract,
}

#[derive(Clone)]
pub struct WasmRuntime {
    engine: Engine,
}

impl Default for WasmRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl WasmRuntime {
    pub fn new() -> Self {
        let mut config = Config::new();
        config.consume_fuel(true);
        let engine = Engine::new(&config).expect("wasm engine");
        Self { engine }
    }

    pub fn compile(&self, wasm_or_wat: &[u8]) -> Result<Vec<u8>, WasmError> {
        // Accept WAT or WASM; always store serialized module bytes as WASM.
        let wasm = if looks_like_wasm(wasm_or_wat) {
            wasm_or_wat.to_vec()
        } else {
            wat::parse_bytes(wasm_or_wat)
                .map(|c| c.into_owned())
                .map_err(|e| WasmError::Compile(e.to_string()))?
        };
        // Validate by compiling
        Module::new(&self.engine, &wasm).map_err(|e| WasmError::Compile(e.to_string()))?;
        Ok(wasm)
    }

    pub fn call_i32_i32(
        &self,
        module_bytes: &[u8],
        export: &str,
        a: i32,
        b: i32,
        fuel: u64,
    ) -> Result<i32, WasmError> {
        let module =
            Module::new(&self.engine, module_bytes).map_err(|e| WasmError::Compile(e.to_string()))?;
        let linker = Linker::new(&self.engine);
        let mut store = Store::new(&self.engine, ());
        store
            .set_fuel(fuel)
            .map_err(|e| WasmError::Call(e.to_string()))?;
        let instance = linker
            .instantiate(&mut store, &module)
            .map_err(|e| WasmError::Instantiate(e.to_string()))?;
        let func = instance
            .get_typed_func::<(i32, i32), i32>(&mut store, export)
            .map_err(|_| WasmError::ExportNotFound(export.to_string()))?;
        match func.call(&mut store, (a, b)) {
            Ok(v) => Ok(v),
            Err(e) => {
                let msg = e.to_string();
                if msg.contains("fuel") || msg.to_lowercase().contains("exhaust") {
                    Err(WasmError::FuelExhausted)
                } else {
                    Err(WasmError::Call(msg))
                }
            }
        }
    }
}

fn looks_like_wasm(bytes: &[u8]) -> bool {
    bytes.len() >= 4 && bytes[0..4] == [0x00, 0x61, 0x73, 0x6d]
}

/// In-memory contract code store used by L1 execute layer.
#[derive(Default)]
pub struct ContractStore {
    pub code: HashMap<[u8; 32], Vec<u8>>,
}

impl ContractStore {
    pub fn insert(&mut self, code_hash: [u8; 32], bytes: Vec<u8>) {
        self.code.insert(code_hash, bytes);
    }

    pub fn get(&self, code_hash: &[u8; 32]) -> Option<&[u8]> {
        self.code.get(code_hash).map(|v| v.as_slice())
    }
}

pub fn code_hash(bytes: &[u8]) -> [u8; 32] {
    *blake3::hash(bytes).as_bytes()
}

// blake3 via re-export dependency - add to Cargo.toml
use blake3;
