import { useState } from "react";
import { demo } from "./native";

export type Todo = { id: string; text: string; done: boolean };
const KEY = demo ? "floatingbar.todos.demo.v1" : "floatingbar.todos.v1";
const LIMIT = 200;

export function useTodos() {
  const [initial] = useState(() => {
    if (demo === "todos") return { items: [
      { id: "demo-1", text: "Review pull request", done: false },
      { id: "demo-2", text: "Siapkan catatan rapat", done: false },
      { id: "demo-3", text: "Rapikan dokumentasi", done: true },
    ], error: null as string | null };
    try {
      const value: unknown = JSON.parse(localStorage.getItem(KEY) ?? "[]");
      if (!Array.isArray(value) || value.length > LIMIT || value.some((t) =>
        !t || typeof t.id !== "string" || typeof t.text !== "string" || typeof t.done !== "boolean"
      )) throw new Error("Data tugas tidak valid");
      return { items: value as Todo[], error: null as string | null };
    } catch {
      return { items: [] as Todo[], error: "Daftar tugas tidak bisa dibaca. Penyimpanan belum diubah." };
    }
  });
  const [items, setItems] = useState(initial.items);
  const [error, setError] = useState(initial.error);
  // Keep in-progress input when a notification temporarily replaces the panel.
  const [draft, setDraft] = useState("");
  const [editing, setEditing] = useState<string | null>(null);
  const [editText, setEditText] = useState("");

  function update(next: Todo[]) {
    try {
      localStorage.setItem(KEY, JSON.stringify(next));
      setItems(next);
      setError(null);
      return true;
    } catch {
      setError("Tugas gagal disimpan. Periksa ruang penyimpanan lalu coba lagi.");
      return false;
    }
  }

  return {
    items, error, draft, setDraft, editing, setEditing, editText, setEditText,
    remaining: items.filter((t) => !t.done).length,
    add(text: string) {
      if (!text.trim()) return false;
      if (items.length >= LIMIT) { setError("Maksimal 200 tugas. Hapus tugas yang sudah selesai."); return false; }
      return update([...items, { id: crypto.randomUUID(), text: text.trim().slice(0, 240), done: false }]);
    },
    toggle(id: string) { update(items.map((t) => t.id === id ? { ...t, done: !t.done } : t)); },
    edit(id: string, text: string) {
      return !!text.trim() && update(items.map((t) => t.id === id ? { ...t, text: text.trim().slice(0, 240) } : t));
    },
    remove(id: string) { update(items.filter((t) => t.id !== id)); },
    clearDone() { update(items.filter((t) => !t.done)); },
  };
}
