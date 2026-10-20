import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

export interface Replacement {
  from: string;
  to: string;
}

export interface VocabularyData {
  terms: string[];
  replacements: Replacement[];
}

export default function Vocabulary() {
  const [terms, setTerms] = useState("");
  const [replacements, setReplacements] = useState<Replacement[]>([]);
  const [status, setStatus] = useState<string | null>(null);

  function apply(v: VocabularyData) {
    setTerms(v.terms.join("\n"));
    setReplacements(v.replacements);
  }

  useEffect(() => {
    invoke<VocabularyData>("vocabulary_get")
      .then(apply)
      .catch((e) => setStatus(String(e)));
  }, []);

  async function save() {
    try {
      const saved = await invoke<VocabularyData>("vocabulary_save", {
        vocabulary: { terms: terms.split("\n"), replacements },
      });
      apply(saved);
      setStatus("Vocabulario guardado");
    } catch (e) {
      setStatus(String(e));
    }
  }

  const update = (i: number, field: keyof Replacement, value: string) =>
    setReplacements(replacements.map((r, j) => (j === i ? { ...r, [field]: value } : r)));

  return (
    <section>
      <h3>Vocabulario</h3>
      <label>
        Términos (uno por línea)
        <textarea
          rows={5}
          value={terms}
          placeholder={"Kubernetes\nAnthropic"}
          onChange={(e) => setTerms(e.target.value)}
        />
      </label>
      <h4>Reemplazos</h4>
      <ul aria-label="Reemplazos">
        {replacements.map((r, i) => (
          <li key={i}>
            <input
              aria-label={`Si oyes ${i + 1}`}
              value={r.from}
              onChange={(e) => update(i, "from", e.target.value)}
            />
            →
            <input
              aria-label={`Escribe ${i + 1}`}
              value={r.to}
              onChange={(e) => update(i, "to", e.target.value)}
            />
            <button onClick={() => setReplacements(replacements.filter((_, j) => j !== i))}>
              Quitar
            </button>
          </li>
        ))}
      </ul>
      <button onClick={() => setReplacements([...replacements, { from: "", to: "" }])}>
        Añadir reemplazo
      </button>
      <button onClick={save}>Guardar vocabulario</button>
      {status && <p role="status">{status}</p>}
    </section>
  );
}
