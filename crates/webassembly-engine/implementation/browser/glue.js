// The thin JavaScript side of the browser engine: each function is one call of the WebAssembly JavaScript API.

const modules = new Map();
const sharedMemories = new Map();
let thrown;

export function compile(bytes) {
  return new WebAssembly.Module(new Uint8Array(bytes));
}

export function registerModule(id, module) {
  modules.set(id, module);
}

export function registeredModule(id) {
  return modules.get(id);
}

export function forgetModule(id) {
  modules.delete(id);
}

export function registerSharedMemory(id, memory) {
  sharedMemories.set(id, memory);
}

export function registeredSharedMemory(id) {
  return sharedMemories.get(id);
}

export function forgetSharedMemory(id) {
  sharedMemories.delete(id);
}

export function newImports() {
  return {};
}

export function addImport(imports, moduleName, itemName, value) {
  (imports[moduleName] ??= {})[itemName] = value;
}

export function instantiate(module, imports) {
  return new WebAssembly.Instance(module, imports);
}

export function instanceExports(instance, name) {
  return Object.hasOwn(instance.exports, name) ? instance.exports[name] : undefined;
}

export function isFunction(value) {
  return typeof value === "function";
}

export function isTable(value) {
  return value instanceof WebAssembly.Table;
}

export function isMemory(value) {
  return value instanceof WebAssembly.Memory;
}

export function isGlobal(value) {
  return value instanceof WebAssembly.Global;
}

// Calls the function and answers whether it returned; what it threw stays for `takeThrown`.
export function call(callee) {
  try {
    callee();
    return true;
  } catch (error) {
    thrown = error;
    return false;
  }
}

// Returns what the last call that did not return threw.
export function takeThrown() {
  const error = thrown;
  thrown = undefined;
  return error;
}

export function isException(error) {
  return typeof WebAssembly.Exception === "function" && error instanceof WebAssembly.Exception;
}

export function instantiateWith(module, namespace, name, value) {
  try {
    return new WebAssembly.Instance(module, { [namespace]: { [name]: value } });
  } catch {
    return undefined;
  }
}

export function instantiateWithTwo(module, namespace, name, value, otherName, otherValue) {
  return new WebAssembly.Instance(module, { [namespace]: { [name]: value, [otherName]: otherValue } });
}

// A 64-bit memory or table takes its sizes as BigInt, a 32-bit one as Number.
function address(value, address64) {
  return address64 ? BigInt(value) : value;
}

export function newMemory(initial, maximum, shared, address64) {
  const descriptor = { initial: address(initial, address64), shared };
  if (maximum !== undefined) descriptor.maximum = address(maximum, address64);
  if (address64) descriptor.address = "i64";
  return new WebAssembly.Memory(descriptor);
}

export function memoryBytes(memory) {
  return memory.buffer.byteLength;
}

export function growMemory(memory, pages, address64) {
  try {
    memory.grow(address(pages, address64));
    return true;
  } catch {
    return false;
  }
}

export function isShared(memory) {
  return typeof SharedArrayBuffer === "function" && memory.buffer instanceof SharedArrayBuffer;
}

export function newTable(initial, maximum, address64) {
  const descriptor = { element: "anyfunc", initial: address(initial, address64) };
  if (maximum !== undefined) descriptor.maximum = address(maximum, address64);
  if (address64) descriptor.address = "i64";
  return new WebAssembly.Table(descriptor);
}

export function tableLength(table) {
  return Number(table.length);
}

// A 64-bit table gives its length as BigInt.
export function isTable64(table) {
  return typeof table.length === "bigint";
}

export function growTable(table, elements, address64) {
  try {
    table.grow(address(elements, address64));
    return true;
  } catch {
    return false;
  }
}

// Returns the element at the index, or undefined, which no element of a funcref table is, when there is none.
export function tableElement(table, index, address64) {
  try {
    return table.get(address(index, address64));
  } catch {
    return undefined;
  }
}

export function setTableElement(table, index, value, address64) {
  try {
    table.set(address(index, address64), value);
    return true;
  } catch {
    return false;
  }
}

export function newI32Global(mutable, value) {
  return new WebAssembly.Global({ value: "i32", mutable }, value);
}

export function newI64Global(mutable, value) {
  return new WebAssembly.Global({ value: "i64", mutable }, value);
}

export function newFunctionGlobal(mutable, value) {
  return new WebAssembly.Global({ value: "anyfunc", mutable }, value);
}

export function numberValue(global) {
  return global.value;
}

export function bigIntValue(global) {
  return global.value;
}

export function referenceValue(global) {
  return global.value;
}

export function setNumberValue(global, value) {
  global.value = value;
}

export function setBigIntValue(global, value) {
  global.value = value;
}

export function setReferenceValue(global, value) {
  global.value = value;
}
