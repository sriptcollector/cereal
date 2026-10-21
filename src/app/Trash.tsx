import { useEffect, useState } from "react";
import { api, Meeting } from "../api";
import type { Go } from "./App";
import { AvatarSq, dayLabel, Icon } from "./ui";

/** Deleted notes stay here for 30 days. */
export default function TrashPage({ go }: { go: Go }) {
  const [list, setList] = useState<Meeting[]>([]);
  const load = () => api.trashList().then(setList);
  useEffect(() => {
    load();
  }, []);
  const daysLeft = (iso: string | null) => (iso ? Math.max(0, 30 - Math.floor((Date.now() - new Date(iso).getTime()) / 86400000)) : 30);
  return (
    <div className="page">
      <div className="topbar" />
      <div className="page-body" style={{ paddingTop: 0 }}>
        <h1 className="h-serif">Trash</h1>
        <p className="sub">Notes you delete stay here for 30 days, then they're gone for good.</p>
        <div className="list">
          {list.map((m) => (
            <div key={m.id} className="item">
              <AvatarSq text={m.title || "U"} />
              <div className="item-main">
                <div className="item-title">{m.title || "Untitled meeting"}</div>
                <div className="item-sub">
                  {dayLabel(m.createdAt)} · deleted, {daysLeft(m.deletedAt)} days left
                </div>
              </div>
              <button className="btn" onClick={() => api.restore(m.id).then(() => go({ page: "meeting", id: m.id }))}>
                Restore
              </button>
              <button className="icon-btn" title="Delete forever" onClick={() => api.purge(m.id).then(load)}>
                {Icon.trash}
              </button>
            </div>
          ))}
          {list.length === 0 && <div className="empty-line">The trash is empty.</div>}
        </div>
      </div>
    </div>
  );
}
