use blockai_wasm::{WasmError, WasmRuntime};

const ADD_WAT: &str = r#"
(module
  (func (export "add") (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.add))
"#;

const LOOP_WAT: &str = r#"
(module
  (func (export "spin") (param i32 i32) (result i32)
    (local $i i32)
    (local.set $i (i32.const 0))
    (loop $l
      (local.set $i (i32.add (local.get $i) (i32.const 1)))
      (br_if $l (i32.lt_u (local.get $i) (i32.const 1000000)))
    )
    local.get $i))
"#;

#[test]
fn add_works_with_fuel() {
    let rt = WasmRuntime::new();
    let code = rt.compile(ADD_WAT.as_bytes()).unwrap();
    let v = rt.call_i32_i32(&code, "add", 2, 40, 10_000).unwrap();
    assert_eq!(v, 42);
}

#[test]
fn fuel_exhaustion_fails_closed() {
    let rt = WasmRuntime::new();
    let code = rt.compile(LOOP_WAT.as_bytes()).unwrap();
    let err = rt.call_i32_i32(&code, "spin", 0, 0, 10).unwrap_err();
    assert!(matches!(err, WasmError::FuelExhausted) || matches!(err, WasmError::Call(_)));
}
