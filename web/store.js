// Where a project lives in a browser. A backend loads a project whole and
// replaces or removes one file at a time; `studio.js` tells it what each
// tool call wrote (`StudioCore.take`) before the call's promise resolves.
//
// Each write replaces the whole file and leaves no half of one: OPFS's
// `createWritable` writes to a copy it swaps in on `close`, an IndexedDB
// `put` is one transaction. Both are what `FsStore` promises with its
// write-then-rename.

// DECISION: OPFS first. It is files and directories, as the project is on
// disk, so a project moves between the two as it is; IndexedDB where a
// browser lacks OPFS's writable streams.
export async function openBackend(name) {
  const root = globalThis.navigator?.storage?.getDirectory && (await navigator.storage.getDirectory().catch(() => null));
  if (root && "createWritable" in FileSystemFileHandle.prototype) return opfsBackend(name, root);
  if (globalThis.indexedDB) return indexedDbBackend(name);
  throw new Error("this browser keeps no files: it has neither OPFS nor IndexedDB");
}

// A project in memory: node, tests, a page that keeps nothing.
export function memoryBackend(files = {}) {
  const held = new Map(Object.entries(files));
  return {
    place: "memory",
    async load() {
      return Object.fromEntries(held);
    },
    async write(path, text) {
      held.set(path, text);
    },
    async remove(path) {
      held.delete(path);
    },
    files: held,
  };
}

// A project in a directory `name` of the origin's private file system.
export async function opfsBackend(name, root) {
  root ??= await navigator.storage.getDirectory();
  const top = await root.getDirectoryHandle(name, { create: true });
  const directory = async (path, create) => {
    let dir = top;
    const parts = path.split("/");
    for (const part of parts.slice(0, -1)) dir = await dir.getDirectoryHandle(part, { create });
    return [dir, parts.at(-1)];
  };
  return {
    place: `opfs:${name}`,
    async load() {
      const files = {};
      const walk = async (dir, prefix) => {
        for await (const [entry, handle] of dir.entries()) {
          if (handle.kind === "directory") await walk(handle, `${prefix}${entry}/`);
          else files[`${prefix}${entry}`] = await (await handle.getFile()).text();
        }
      };
      await walk(top, "");
      return files;
    },
    async write(path, text) {
      const [dir, file] = await directory(path, true);
      const writable = await (await dir.getFileHandle(file, { create: true })).createWritable();
      try {
        await writable.write(text);
        await writable.close();
      } catch (e) {
        // Nothing of an aborted stream reaches the file.
        await writable.abort().catch(() => {});
        throw e;
      }
    },
    async remove(path) {
      const [dir, file] = await directory(path, false);
      await dir.removeEntry(file);
    },
  };
}

// A project in an IndexedDB database `ljos-studio:name`, a record per file.
export async function indexedDbBackend(name) {
  const db = await request(
    Object.assign(indexedDB.open(`ljos-studio:${name}`, 1), {
      onupgradeneeded: (e) => e.target.result.createObjectStore("files"),
    }),
  );
  const files = (mode) => db.transaction("files", mode).objectStore("files");
  const done = (store) =>
    new Promise((resolve, reject) => {
      store.transaction.oncomplete = resolve;
      store.transaction.onerror = store.transaction.onabort = () => reject(store.transaction.error);
    });
  return {
    place: `indexeddb:${name}`,
    async load() {
      const store = files("readonly");
      const [keys, values] = await Promise.all([request(store.getAllKeys()), request(store.getAll())]);
      return Object.fromEntries(keys.map((key, i) => [key, values[i]]));
    },
    async write(path, text) {
      const store = files("readwrite");
      store.put(text, path);
      await done(store);
    },
    async remove(path) {
      const store = files("readwrite");
      store.delete(path);
      await done(store);
    },
  };
}

function request(req) {
  return new Promise((resolve, reject) => {
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => reject(req.error);
  });
}
