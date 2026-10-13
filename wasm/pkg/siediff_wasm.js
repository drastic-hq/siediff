/* @ts-self-types="./siediff_wasm.d.ts" */

/**
 * @param {Uint8Array} left
 * @param {Uint8Array} right
 * @param {boolean} right_is_snapshot
 * @param {boolean} renumbered
 * @returns {string}
 */
export function compare_inputs(left, right, right_is_snapshot, renumbered) {
  let deferred4_0;
  let deferred4_1;

  try {
    const ptr0 = passArray8ToWasm0(left, wasm.__wbindgen_malloc);
    const len0 = WASM_VECTOR_LEN;
    const ptr1 = passArray8ToWasm0(right, wasm.__wbindgen_malloc);
    const len1 = WASM_VECTOR_LEN;
    const ret = wasm.compare_inputs(ptr0, len0, ptr1, len1, right_is_snapshot, renumbered);
    var ptr3 = ret[0];
    var len3 = ret[1];

    if (ret[3]) {
      ptr3 = 0;
      len3 = 0;
      throw takeFromExternrefTable0(ret[2]);
    }

    deferred4_0 = ptr3;
    deferred4_1 = len3;

    return getStringFromWasm0(ptr3, len3);
  } finally {
    wasm.__wbindgen_free(deferred4_0, deferred4_1, 1);
  }
}

function __wbg_get_imports() {
  const import0 = {
    __proto__: null,
    __wbindgen_generic_0000000000000001: function (arg0, arg1) {
      // Cast intrinsic for `Ref(String) -> Externref`.
      const ret = getStringFromWasm0(arg0, arg1);

      return ret;
    },
    __wbindgen_init_externref_table: function () {
      const table = wasm.__wbindgen_externrefs;
      const offset = table.grow(4);
      table.set(0, undefined);
      table.set(offset + 0, undefined);
      table.set(offset + 1, null);
      table.set(offset + 2, true);
      table.set(offset + 3, false);
    },
  };

  return {
    __proto__: null,
    "./siediff_wasm_bg.js": import0,
  };
}

function getStringFromWasm0(ptr, len) {
  return decodeText(ptr >>> 0, len);
}

let cachedUint8ArrayMemory0 = null;

function getUint8ArrayMemory0() {
  if (cachedUint8ArrayMemory0 === null || cachedUint8ArrayMemory0.byteLength === 0) {
    cachedUint8ArrayMemory0 = new Uint8Array(wasm.memory.buffer);
  }

  return cachedUint8ArrayMemory0;
}

function passArray8ToWasm0(arg, malloc) {
  const ptr = malloc(arg.length * 1, 1) >>> 0;
  getUint8ArrayMemory0().set(arg, ptr / 1);
  WASM_VECTOR_LEN = arg.length;

  return ptr;
}

function takeFromExternrefTable0(idx) {
  const value = wasm.__wbindgen_externrefs.get(idx);
  wasm.__externref_table_dealloc(idx);

  return value;
}

let cachedTextDecoder = new TextDecoder("utf-8", { ignoreBOM: true, fatal: true });

cachedTextDecoder.decode();

const MAX_SAFARI_DECODE_BYTES = 2146435072;

let numBytesDecoded = 0;

function decodeText(ptr, len) {
  numBytesDecoded += len;

  if (numBytesDecoded >= MAX_SAFARI_DECODE_BYTES) {
    cachedTextDecoder = new TextDecoder("utf-8", { ignoreBOM: true, fatal: true });
    cachedTextDecoder.decode();
    numBytesDecoded = len;
  }

  return cachedTextDecoder.decode(getUint8ArrayMemory0().subarray(ptr, ptr + len));
}

let WASM_VECTOR_LEN = 0;

let wasm;

function __wbg_finalize_init(instance) {
  wasm = instance.exports;
  cachedUint8ArrayMemory0 = null;
  wasm.__wbindgen_start();

  return wasm;
}

export function initSync({ module }) {
  const instance = new WebAssembly.Instance(module, __wbg_get_imports());

  return __wbg_finalize_init(instance);
}
