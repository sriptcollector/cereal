import { useState } from "react";
import { api, Recipe } from "../api";
import { Icon, toast, useRecipes } from "./ui";

const EMPTY: Recipe = { id: "", cmd: "", name: "", prompt: "", kind: "single", builtin: false };

/** Saved prompts you run with "/" in any chat box. */
export default function RecipesPage() {
  const [recipes, reload] = useRecipes();
  const [draft, setDraft] = useState<Recipe | null>(null);
  const save = async () => {
    if (!draft) return;
    if (!draft.name.trim() || !draft.prompt.trim()) return toast("Give it a name and a prompt");
    const r = await api.saveRecipe({ ...draft, cmd: draft.cmd || draft.name });
    setDraft(r);
    reload();
    toast(`Saved. Type /${r.cmd} in any chat.`);
  };
  return (
    <div className="page">
      <div className="topbar">
        <div className="spacer" />
        <button className="btn" onClick={() => setDraft({ ...EMPTY })}>
          {Icon.plus} New recipe
        </button>
      </div>
      <div className="page-body wide" style={{ paddingTop: 0 }}>
        <h1 className="h-serif">Recipes</h1>
        <p className="sub">Saved prompts. Type / in any chat box to use one. One-meeting recipes show up on a note; multi-meeting ones on Home, folders, people and companies.</p>
        <div className="tpl-grid">
          <div className="tpl-list">
            {recipes.map((r) => (
              <button key={r.id} className={`tpl ${draft?.id === r.id ? "on" : ""}`} onClick={() => setDraft({ ...r })}>
                <div className="tpl-name">
                  <span className="muted">/</span>
                  {r.cmd} {!r.builtin && <span className="chip">Yours</span>}
                </div>
                <div className="tpl-desc">{r.name}</div>
              </button>
            ))}
          </div>
          <div>
            {draft ? (
              <div className="card pad">
                <label className="field">
                  <span>Name</span>
                  <input value={draft.name} onChange={(e) => setDraft({ ...draft, name: e.target.value })} placeholder="Client update" disabled={draft.builtin} />
                </label>
                <label className="field">
                  <span>Shortcut (what you type after /)</span>
                  <input value={draft.cmd} onChange={(e) => setDraft({ ...draft, cmd: e.target.value })} placeholder="client-update" disabled={draft.builtin} />
                </label>
                <label className="field">
                  <span>Prompt</span>
                  <textarea rows={6} value={draft.prompt} onChange={(e) => setDraft({ ...draft, prompt: e.target.value })} disabled={draft.builtin} />
                </label>
                <label className="field">
                  <span>Works on</span>
                  <select value={draft.kind} onChange={(e) => setDraft({ ...draft, kind: e.target.value as Recipe["kind"] })} disabled={draft.builtin}>
                    <option value="single">One meeting</option>
                    <option value="multi">Several meetings</option>
                  </select>
                </label>
                <div style={{ display: "flex", gap: 8 }}>
                  {draft.builtin ? (
                    <button className="btn" onClick={() => setDraft({ ...draft, id: "", builtin: false, name: `${draft.name} (mine)`, cmd: `${draft.cmd}-2` })}>
                      {Icon.copy} Make my own version
                    </button>
                  ) : (
                    <>
                      {draft.id && (
                        <button className="btn ghost danger" onClick={() => api.deleteRecipe(draft.id).then(() => (setDraft(null), reload()))}>
                          {Icon.trash} Delete
                        </button>
                      )}
                      <div className="spacer" />
                      <button className="btn dark" onClick={save}>
                        Save recipe
                      </button>
                    </>
                  )}
                </div>
              </div>
            ) : (
              <div className="empty-line">Pick a recipe to see its prompt, or make your own.</div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
