import { useEffect, useState, type KeyboardEvent } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ArrowIcon, CloseIcon, PlusIcon, TrashIcon } from "./icons";

export interface Replacement {
  from: string;
  to: string;
}

export interface VocabularyData {
  terms: string[];
  replacements: Replacement[];
}

/** Añade un término si no está vacío ni repetido (sin distinguir mayúsculas). */
export function addTerm(terms: string[], raw: string): string[] {
  const term = raw.trim();
  if (!term || terms.some((t) => t.toLowerCase() === term.toLowerCase())) return terms;
  return [...terms, term];
}

export default function Vocabulary() {
  const [terms, setTerms] = useState<string[]>([]);
  const [draft, setDraft] = useState("");
  const [replacements, setReplacements] = useState<Replacement[]>([]);
  const [status, setStatus] = useState<{ ok: boolean; text: string } | null>(null);

  function apply(v: VocabularyData) {
    setTerms(v.terms);
    setReplacements(v.replacements);
  }

  useEffect(() => {
    invoke<VocabularyData>("vocabulary_get")
      .then(apply)
      .catch((e) => setStatus({ ok: false, text: String(e) }));
  }, []);

  async function save() {
    try {
      const saved = await invoke<VocabularyData>("vocabulary_save", {
        vocabulary: { terms: addTerm(terms, draft), replacements },
      });
      apply(saved);
      setDraft("");
      setStatus({ ok: true, text: "Guardado" });
    } catch (e) {
      setStatus({ ok: false, text: String(e) });
    }
  }

  function onTermKey(e: KeyboardEvent<HTMLInputElement>) {
    if (e.key === "Enter") {
      e.preventDefault();
      setTerms(addTerm(terms, draft));
      setDraft("");
    } else if (e.key === "Backspace" && draft === "" && terms.length > 0) {
      setTerms(terms.slice(0, -1));
    }
  }

  const update = (i: number, field: keyof Replacement, value: string) =>
    setReplacements(replacements.map((r, j) => (j === i ? { ...r, [field]: value } : r)));

  return (
    <main className="page" style={{ gap: 22 }}>
      <div className="page-head">
        <h1 className="page-title">Vocabulario</h1>
        <p className="page-lead">Nombres y términos que quieres que se escriban siempre bien.</p>
      </div>
      <section aria-labelledby="v-terms" className="stack-10">
        <h2 id="v-terms" className="section-title">
          Términos
        </h2>
        <div className="terms">
          {terms.map((t) => (
            <span key={t} className="term">
              {t}
              <button
                type="button"
                aria-label={`Quitar ${t}`}
                onClick={() => setTerms(terms.filter((x) => x !== t))}
              >
                <CloseIcon size={12} />
              </button>
            </span>
          ))}
          <input
            aria-label="Añadir término"
            placeholder="Añadir término y pulsar Enter"
            value={draft}
            onChange={(e) => setDraft(e.target.value)}
            onKeyDown={onTermKey}
          />
        </div>
      </section>
      <section aria-labelledby="v-repl" className="stack-10">
        <div className="section-head">
          <h2 id="v-repl" className="section-title">
            Reemplazos
          </h2>
          <span className="section-note">
            Se aplican después de transcribir, en todos los modos.
          </span>
        </div>
        <div className="table">
          <div className="repl-row head">
            <span>Si oyes</span>
            <span />
            <span>Escribe</span>
            <span />
          </div>
          {replacements.map((r, i) => (
            <div key={i} className="repl-row">
              <input
                className="input"
                aria-label={`Si oyes ${i + 1}`}
                value={r.from}
                onChange={(e) => update(i, "from", e.target.value)}
              />
              <ArrowIcon size={20} stroke="var(--idle)" />
              <input
                className="input"
                aria-label={`Escribe ${i + 1}`}
                value={r.to}
                onChange={(e) => update(i, "to", e.target.value)}
              />
              <button
                type="button"
                className="btn btn-ghost-icon"
                aria-label={`Quitar reemplazo ${i + 1}`}
                onClick={() => setReplacements(replacements.filter((_, j) => j !== i))}
              >
                <TrashIcon size={16} />
              </button>
            </div>
          ))}
          <div className="table-foot">
            <button
              type="button"
              className="btn btn-link"
              onClick={() => setReplacements([...replacements, { from: "", to: "" }])}
            >
              <PlusIcon size={14} />
              Añadir reemplazo
            </button>
          </div>
        </div>
      </section>
      <div className="grow" />
      <div className="save-bar">
        {status && (
          <span
            role="status"
            className={status.ok ? "save-status ok-text" : "save-status error-text"}
          >
            {status.text}
          </span>
        )}
        <button type="button" className="btn btn-primary" onClick={save}>
          Guardar vocabulario
        </button>
      </div>
    </main>
  );
}
