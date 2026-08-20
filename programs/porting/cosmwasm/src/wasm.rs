//! Deterministic `WebAssembly` module assembly for ported programs.
//!
//! The kit compiles a port descriptor straight to the deterministic subset the
//! programs runtime admits: integer instructions, one linear memory, constant
//! data segments and imports drawn only from the `layerx_v1` module. A
//! `CosmWasm` contract is already `WebAssembly`, but it is built against the
//! `wasmd` import set and a `JSON` message boundary; nothing of it survives
//! except its meaning, so the port is emitted rather than relinked. The same
//! descriptor always produces the same bytes, which is what makes the emitted
//! artifact reproducible against its published source.

/// The `i32` value-type code.
pub const I32: u8 = 0x7f;
/// The `i64` value-type code.
pub const I64: u8 = 0x7e;
/// The empty block-type code.
pub const VOID_BLOCK: u8 = 0x40;

/// The `unreachable` opcode, which is a ported `panic!` or returned `Err`.
pub const UNREACHABLE: u8 = 0x00;
/// The `block` opcode.
pub const BLOCK: u8 = 0x02;
/// The `loop` opcode.
pub const LOOP: u8 = 0x03;
/// The `if` opcode.
pub const IF: u8 = 0x04;
/// The `else` opcode.
pub const ELSE: u8 = 0x05;
/// The `end` opcode.
pub const END: u8 = 0x0b;
/// The `br` opcode.
pub const BR: u8 = 0x0c;
/// The `br_if` opcode.
pub const BR_IF: u8 = 0x0d;
/// The `return` opcode.
pub const RETURN: u8 = 0x0f;
/// The `drop` opcode.
pub const DROP: u8 = 0x1a;
/// The `i32.load` opcode.
pub const I32_LOAD: u8 = 0x28;
/// The `i64.load` opcode.
pub const I64_LOAD: u8 = 0x29;
/// The `i32.load8_u` opcode.
pub const I32_LOAD8_U: u8 = 0x2d;
/// The `i32.load16_u` opcode.
pub const I32_LOAD16_U: u8 = 0x2f;
/// The `i32.store` opcode.
pub const I32_STORE: u8 = 0x36;
/// The `i64.store` opcode.
pub const I64_STORE: u8 = 0x37;
/// The `i32.store8` opcode.
pub const I32_STORE8: u8 = 0x3a;
/// The `i32.store16` opcode.
pub const I32_STORE16: u8 = 0x3b;
/// The `i32.eqz` opcode.
pub const I32_EQZ: u8 = 0x45;
/// The `i32.eq` opcode.
pub const I32_EQ: u8 = 0x46;
/// The `i32.ne` opcode.
pub const I32_NE: u8 = 0x47;
/// The `i32.lt_s` opcode.
pub const I32_LT_S: u8 = 0x48;
/// The `i32.lt_u` opcode.
pub const I32_LT_U: u8 = 0x49;
/// The `i32.gt_s` opcode.
pub const I32_GT_S: u8 = 0x4a;
/// The `i32.gt_u` opcode.
pub const I32_GT_U: u8 = 0x4b;
/// The `i32.ge_u` opcode.
pub const I32_GE_U: u8 = 0x4f;
/// The `i64.eqz` opcode.
pub const I64_EQZ: u8 = 0x50;
/// The `i64.eq` opcode.
pub const I64_EQ: u8 = 0x51;
/// The `i64.ne` opcode.
pub const I64_NE: u8 = 0x52;
/// The `i64.lt_s` opcode.
pub const I64_LT_S: u8 = 0x53;
/// The `i64.gt_s` opcode.
pub const I64_GT_S: u8 = 0x55;
/// The `i64.gt_u` opcode.
pub const I64_GT_U: u8 = 0x56;
/// The `i32.add` opcode.
pub const I32_ADD: u8 = 0x6a;
/// The `i32.sub` opcode.
pub const I32_SUB: u8 = 0x6b;
/// The `i64.add` opcode.
pub const I64_ADD: u8 = 0x7c;
/// The `i64.sub` opcode.
pub const I64_SUB: u8 = 0x7d;
/// The `i64.mul` opcode.
pub const I64_MUL: u8 = 0x7e;
/// The `i64.or` opcode.
pub const I64_OR: u8 = 0x84;
/// The `i64.shl` opcode.
pub const I64_SHL: u8 = 0x86;
/// The `i64.shr_u` opcode.
pub const I64_SHR_U: u8 = 0x88;
/// The `i32.wrap_i64` opcode.
pub const I32_WRAP_I64: u8 = 0xa7;
/// The `i64.extend_i32_u` opcode.
pub const I64_EXTEND_I32_U: u8 = 0xad;

const FUNCTION_TYPE: u8 = 0x60;
const KIND_FUNCTION: u8 = 0x00;
const KIND_MEMORY: u8 = 0x02;
const SECTION_TYPE: u8 = 1;
const SECTION_IMPORT: u8 = 2;
const SECTION_FUNCTION: u8 = 3;
const SECTION_MEMORY: u8 = 5;
const SECTION_EXPORT: u8 = 7;
const SECTION_CODE: u8 = 10;
const SECTION_DATA: u8 = 11;

/// One function body under construction.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Code {
    bytes: Vec<u8>,
}

impl Code {
    /// Starts an empty body.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends one operand-free opcode.
    pub fn op(&mut self, opcode: u8) {
        self.bytes.push(opcode);
    }

    /// Appends `i32.const`.
    pub fn i32_const(&mut self, value: i32) {
        self.bytes.push(0x41);
        signed_leb(i64::from(value), &mut self.bytes);
    }

    /// Appends `i64.const`.
    pub fn i64_const(&mut self, value: i64) {
        self.bytes.push(0x42);
        signed_leb(value, &mut self.bytes);
    }

    /// Appends a linear-memory address as `i32.const`.
    pub fn pointer(&mut self, address: u32) {
        self.i32_const(i32::try_from(address).unwrap_or(i32::MAX));
    }

    /// Appends `local.get`.
    pub fn local_get(&mut self, index: u32) {
        self.bytes.push(0x20);
        unsigned_leb(u64::from(index), &mut self.bytes);
    }

    /// Appends `local.set`.
    pub fn local_set(&mut self, index: u32) {
        self.bytes.push(0x21);
        unsigned_leb(u64::from(index), &mut self.bytes);
    }

    /// Appends `local.tee`.
    pub fn local_tee(&mut self, index: u32) {
        self.bytes.push(0x22);
        unsigned_leb(u64::from(index), &mut self.bytes);
    }

    /// Appends a load or store with a byte-aligned memory argument.
    pub fn memory(&mut self, opcode: u8, offset: u32) {
        self.bytes.push(opcode);
        unsigned_leb(0, &mut self.bytes);
        unsigned_leb(u64::from(offset), &mut self.bytes);
    }

    /// Appends `call`.
    pub fn call(&mut self, function: u32) {
        self.bytes.push(0x10);
        unsigned_leb(u64::from(function), &mut self.bytes);
    }

    /// Appends `block`, `loop` or `if` with an explicit block type.
    pub fn block(&mut self, opcode: u8, block_type: u8) {
        self.bytes.push(opcode);
        self.bytes.push(block_type);
    }

    /// Appends `br` or `br_if` with a label depth.
    pub fn branch(&mut self, opcode: u8, depth: u32) {
        self.bytes.push(opcode);
        unsigned_leb(u64::from(depth), &mut self.bytes);
    }

    /// Appends `end`.
    pub fn end(&mut self) {
        self.bytes.push(END);
    }

    /// Appends `unreachable`, the ported instruction failure: every staged
    /// write and effect of the whole invocation is discarded.
    pub fn trap(&mut self) {
        self.op(UNREACHABLE);
    }

    /// Appends the ported `require!` failure: trap when the condition on the
    /// stack is nonzero, discarding every staged write and effect.
    pub fn trap_if(&mut self) {
        self.block(IF, VOID_BLOCK);
        self.trap();
        self.end();
    }

    /// Appends the ported status check: trap unless the returned host status
    /// is exactly zero.
    pub fn trap_unless_ok(&mut self) {
        self.i32_const(0);
        self.op(I32_NE);
        self.trap_if();
    }

    /// Returns the encoded instruction bytes.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

/// An assembled module: types, `layerx_v1` imports, one memory, constant data
/// segments, function bodies and exports.
#[derive(Clone, Debug, Default)]
pub struct ModuleBuilder {
    signatures: Vec<(Vec<u8>, Vec<u8>)>,
    imports: Vec<(String, String, u32)>,
    functions: Vec<u32>,
    bodies: Vec<Vec<u8>>,
    exports: Vec<(String, u8, u32)>,
    memory_pages: u32,
    segments: Vec<(u32, Vec<u8>)>,
}

impl ModuleBuilder {
    /// Starts a module owning one linear memory of `memory_pages` pages.
    ///
    /// Declare every import before the first function: function indices count
    /// imports first, exactly as the binary format does.
    #[must_use]
    pub fn new(memory_pages: u32) -> Self {
        Self {
            memory_pages,
            ..Self::default()
        }
    }

    /// Interns a function type and returns its type index.
    pub fn signature(&mut self, params: &[u8], results: &[u8]) -> u32 {
        let entry = (params.to_vec(), results.to_vec());
        if let Some(index) = self
            .signatures
            .iter()
            .position(|existing| existing == &entry)
        {
            return u32::try_from(index).unwrap_or(0);
        }
        self.signatures.push(entry);
        u32::try_from(self.signatures.len().saturating_sub(1)).unwrap_or(0)
    }

    /// Declares one host import and returns its function index.
    pub fn import(&mut self, module: &str, name: &str, signature: u32) -> u32 {
        self.imports
            .push((module.to_string(), name.to_string(), signature));
        u32::try_from(self.imports.len().saturating_sub(1)).unwrap_or(0)
    }

    /// Defines one function body and returns its function index.
    pub fn function(&mut self, signature: u32, locals: &[(u32, u8)], code: &Code) -> u32 {
        let mut body = Vec::with_capacity(code.bytes().len() + 8);
        unsigned_leb(length(locals.len()), &mut body);
        for (count, value_type) in locals {
            unsigned_leb(u64::from(*count), &mut body);
            body.push(*value_type);
        }
        body.extend_from_slice(code.bytes());
        self.functions.push(signature);
        self.bodies.push(body);
        let index = self
            .imports
            .len()
            .saturating_add(self.bodies.len())
            .saturating_sub(1);
        u32::try_from(index).unwrap_or(0)
    }

    /// Exports a function under a name.
    pub fn export_function(&mut self, name: &str, function: u32) {
        self.exports
            .push((name.to_string(), KIND_FUNCTION, function));
    }

    /// Exports the module's linear memory, which the host requires to read and
    /// write guest buffers.
    pub fn export_memory(&mut self, name: &str) {
        self.exports.push((name.to_string(), KIND_MEMORY, 0));
    }

    /// Places constant bytes at a fixed linear-memory address.
    pub fn segment(&mut self, address: u32, bytes: &[u8]) {
        self.segments.push((address, bytes.to_vec()));
    }

    /// Encodes the complete module.
    #[must_use]
    pub fn finish(&self) -> Vec<u8> {
        let mut module = vec![0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00];
        let mut payload = Vec::new();
        unsigned_leb(length(self.signatures.len()), &mut payload);
        for (params, results) in &self.signatures {
            payload.push(FUNCTION_TYPE);
            unsigned_leb(length(params.len()), &mut payload);
            payload.extend_from_slice(params);
            unsigned_leb(length(results.len()), &mut payload);
            payload.extend_from_slice(results);
        }
        section(&mut module, SECTION_TYPE, &payload);
        payload = Vec::new();
        unsigned_leb(length(self.imports.len()), &mut payload);
        for (import_module, import_name, signature) in &self.imports {
            text(import_module, &mut payload);
            text(import_name, &mut payload);
            payload.push(KIND_FUNCTION);
            unsigned_leb(u64::from(*signature), &mut payload);
        }
        section(&mut module, SECTION_IMPORT, &payload);
        payload = Vec::new();
        unsigned_leb(length(self.functions.len()), &mut payload);
        for signature in &self.functions {
            unsigned_leb(u64::from(*signature), &mut payload);
        }
        section(&mut module, SECTION_FUNCTION, &payload);
        payload = vec![1, 0x00];
        unsigned_leb(u64::from(self.memory_pages), &mut payload);
        section(&mut module, SECTION_MEMORY, &payload);
        payload = Vec::new();
        unsigned_leb(length(self.exports.len()), &mut payload);
        for (name, kind, index) in &self.exports {
            text(name, &mut payload);
            payload.push(*kind);
            unsigned_leb(u64::from(*index), &mut payload);
        }
        section(&mut module, SECTION_EXPORT, &payload);
        payload = Vec::new();
        unsigned_leb(length(self.bodies.len()), &mut payload);
        for body in &self.bodies {
            unsigned_leb(length(body.len()), &mut payload);
            payload.extend_from_slice(body);
        }
        section(&mut module, SECTION_CODE, &payload);
        payload = Vec::new();
        unsigned_leb(length(self.segments.len()), &mut payload);
        for (address, bytes) in &self.segments {
            unsigned_leb(0, &mut payload);
            payload.push(0x41);
            signed_leb(i64::from(*address), &mut payload);
            payload.push(END);
            unsigned_leb(length(bytes.len()), &mut payload);
            payload.extend_from_slice(bytes);
        }
        section(&mut module, SECTION_DATA, &payload);
        module
    }
}

fn length(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

fn text(value: &str, out: &mut Vec<u8>) {
    unsigned_leb(length(value.len()), out);
    out.extend_from_slice(value.as_bytes());
}

fn section(module: &mut Vec<u8>, id: u8, payload: &[u8]) {
    module.push(id);
    unsigned_leb(length(payload.len()), module);
    module.extend_from_slice(payload);
}

fn unsigned_leb(mut value: u64, out: &mut Vec<u8>) {
    loop {
        let byte = u8::try_from(value & 0x7f).unwrap_or(0);
        value >>= 7;
        if value == 0 {
            out.push(byte);
            return;
        }
        out.push(byte | 0x80);
    }
}

fn signed_leb(mut value: i64, out: &mut Vec<u8>) {
    loop {
        let byte = u8::try_from(value & 0x7f).unwrap_or(0);
        value >>= 7;
        let sign_set = byte & 0x40 != 0;
        if (value == 0 && !sign_set) || (value == -1 && sign_set) {
            out.push(byte);
            return;
        }
        out.push(byte | 0x80);
    }
}
