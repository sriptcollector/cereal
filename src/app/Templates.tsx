import { useState } from "react";
import { api, Template } from "../api";
import { Icon, Page, toast, useSettings, useTemplates } from "./ui";

const EMPTY: Template = { id: "", name: "", description: "", sections: "Key points\nDecisions\nAction items: owner and due date", instructions: "", builtin: false, updatedAt: "" };

export default function Templates() {
  const [templates, reload] = useTemplates();
  const [settings, save] = useSettings();
  const [sel, setSel] = useState<Template | null>(null);
  const [draft, setDraft] = useState<Template | null>(null);

  const open = (t: Template) => {
    setSel(t);
    setDraft(t.builtin ? null : { ...t });
  };
  const store = async () => {
    if (!draft) return;
    if (!draft.name.trim()) return toast("Give the template a name");
    const t = await api.saveTemplate(draft);
    reload();
    setSel(t);
    setDraft({ ...t });
    toast("Template saved");
  };

  return (
    <Page
      title="Templates"
      sub="How the AI lays out your notes. Pick one per meeting from the template button, or set a default."
      actions={
        <button className="btn primary" onClick={() => (setSel(null), setDraft({ ...EMPTY }))}>
          {Icon.plus} New template
        </button>
      }
    >
      <div className="tpl-grid">
        <div className="tpl-list">
          {templates.map((t) => (
            <button key={t.id} className={`tpl ${sel?.id === t.id ? "on" : ""}`} onClick={() => open(t)}>
              <div className="tpl-name">
                {t.name}
                {settings?.default_template === t.id && <span className="chip">Default</span>}
                {!t.builtin && <span className="chip">Yours</span>}
              </div>
              <div className="tpl-desc">{t.description}</div>
            </button>
          ))}
        </div>
        <div className="tpl-detail">
          {draft ? (
            <div className="card pad">
              <label className="field">
                <span>Name</span>
                <input value={draft.name} onChange={(e) => setDraft({ ...draft, name: e.target.value })} placeholder="e.g. Client check-in" />
              </label>
              <label className="field">
                <span>When to use it</span>
                <input value={draft.description} onChange={(e) => setDraft({ ...draft, description: e.target.value })} placeholder="Weekly calls with clients" />
              </label>
              <label className="field">
                <span>Sections, one per line ("Heading: what goes there")</span>
                <textarea rows={8} value={draft.sections} onChange={(e) => setDraft({ ...draft, sections: e.target.value })} />
              </label>
              <label className="field">
                <span>Extra instructions for the AI (optional)</span>
                <textarea rows={3} value={draft.instructions} onChange={(e) => setDraft({ ...draft, instructions: e.target.value })} placeholder="Keep it short. Quote the client's exact words for requests." />
              </label>
              <div className="actions">
                {draft.id && (
                  <button
                    className="btn ghost danger"
                    onClick={async () => {
                      await api.deleteTemplate(draft.id);
                      if (settings?.default_template === draft.id) await save({ default_template: "auto" });
                      setDraft(null);
                      setSel(null);
                      reload();
                    }}
                  >
                    {Icon.trash} Delete
                  </button>
                )}
                <div className="top-spacer" />
                {draft.id && settings?.default_template !== draft.id && (
                  <button className="btn ghost" onClick={() => save({ default_template: draft.id })}>
                    Use by default
                  </button>
                )}
                <button className="btn primary" onClick={store}>
                  Save template
                </button>
              </div>
            </div>
          ) : sel ? (
            <div className="card pad">
              <h2 className="tpl-title">{sel.name}</h2>
              <p className="muted">{sel.description}</p>
              {sel.sections ? (
                <ul className="tpl-sections">
                  {sel.sections.split("\n").filter(Boolean).map((l) => {
                    const [h, ...rest] = l.split(":");
                    return (
                      <li key={l}>
                        <b>{h}</b>
                        {rest.length ? `: ${rest.join(":")}` : ""}
                      </li>
                    );
                  })}
                </ul>
              ) : (
                <p className="muted">The AI picks sections that fit the conversation, then adds decisions and action items.</p>
              )}
              {sel.instructions && <p className="muted small">{sel.instructions}</p>}
              <div className="actions">
                <div className="top-spacer" />
                {settings?.default_template !== sel.id && (
                  <button className="btn ghost" onClick={() => save({ default_template: sel.id })}>
                    Use by default
                  </button>
                )}
                <button className="btn" onClick={() => setDraft({ ...sel, id: "", name: `${sel.name} (my version)`, builtin: false })}>
                  {Icon.copy} Duplicate and edit
                </button>
              </div>
            </div>
          ) : (
            <div className="empty-line">Pick a template to see its sections, or make your own.</div>
          )}
        </div>
      </div>
    </Page>
  );
}
