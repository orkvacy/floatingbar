import { CheckListIcon, CloseIcon, EditIcon, TrashIcon } from "../icons";
import type { useTodos } from "../lib/todos";

export function TodoPanel({ todos, onClose }: { todos: ReturnType<typeof useTodos>; onClose: () => void }) {
  const { draft, setDraft, editing, setEditing, editText, setEditText } = todos;
  const done = todos.items.length - todos.remaining;
  return (
    <section className="organizer" aria-label="Daftar tugas">
      <div className="organizer__head">
        <div><h2><CheckListIcon />To-do list</h2><p>{todos.remaining ? `${todos.remaining} tugas tersisa` : todos.items.length ? "Semua tugas selesai. Kerja bagus!" : "Ruang untuk rencana kecilmu"}</p></div>
        <button className="organizer__icon" onClick={onClose} title="Tutup to-do list" aria-label="Tutup to-do list"><CloseIcon /></button>
      </div>
      <form className="todo__add" onSubmit={(e) => { e.preventDefault(); if (todos.add(draft)) setDraft(""); }}>
        <input aria-label="Tugas baru" placeholder="Apa yang ingin kamu selesaikan?" maxLength={240} value={draft} onChange={(e) => setDraft(e.target.value)} />
        <button className="organizer__primary" type="submit" disabled={!draft.trim()}>Tambah</button>
      </form>
      {todos.error && <p className="organizer__error" role="alert">{todos.error}</p>}
      <ul className="organizer__list todo__list">
        {todos.items.length === 0 ? <li className="organizer__empty"><CheckListIcon /><strong>Mulai dari satu tugas</strong><span>Tulis hal kecil yang ingin kamu bereskan hari ini.</span></li> :
          [...todos.items].sort((a, b) => Number(a.done) - Number(b.done)).map((todo) => (
            <li key={todo.id} className={`todo__row ${todo.done ? "todo__row--done" : ""}`}>
              <input type="checkbox" checked={todo.done} onChange={() => todos.toggle(todo.id)} aria-label={`${todo.done ? "Tandai belum selesai" : "Selesaikan"}: ${todo.text}`} />
              {editing === todo.id ? (
                <form className="todo__edit" onSubmit={(e) => { e.preventDefault(); if (todos.edit(todo.id, editText)) setEditing(null); }}>
                  <input aria-label="Edit tugas" autoFocus value={editText} maxLength={240} onChange={(e) => setEditText(e.target.value)} onKeyDown={(e) => { if (e.key === "Escape") setEditing(null); }} />
                  <button className="organizer__text" type="submit" disabled={!editText.trim()}>Simpan</button>
                  <button className="organizer__icon" type="button" aria-label="Batal edit" onClick={() => setEditing(null)}><CloseIcon /></button>
                </form>
              ) : <><span className="todo__text">{todo.text}</span>
                <button className="organizer__icon" title="Edit tugas" aria-label={`Edit ${todo.text}`} onClick={() => { setEditing(todo.id); setEditText(todo.text); }}><EditIcon /></button>
                <button className="organizer__icon" title="Hapus tugas" aria-label={`Hapus ${todo.text}`} onClick={() => todos.remove(todo.id)}><TrashIcon /></button></>}
            </li>
          ))}
      </ul>
      <div className="organizer__footer"><span>{done} dari {todos.items.length} selesai · Tersimpan di perangkat</span>{done > 0 && <button className="organizer__text" onClick={todos.clearDone}>Bersihkan selesai</button>}</div>
    </section>
  );
}
