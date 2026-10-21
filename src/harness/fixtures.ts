// Datos de ejemplo del arnés de pantallas (solo desarrollo): el mismo contenido que
// muestran los diseños aprobados, para que el oráculo visual compare like con like.

function today(h: number, m: number, daysAgo = 0): number {
  const d = new Date();
  d.setDate(d.getDate() - daysAgo);
  d.setHours(h, m, 0, 0);
  return d.getTime();
}

export const settings = {
  version: 1,
  general: { autostart: true },
  hotkey: { shortcut: "Option+Space", mode: "toggle" },
  audio: { device: null, max_duration_secs: 300 },
  transcription: { model: "whisper-large-v3-turbo", language: null },
  ai: { cleanup: true },
  output: { copy_only: false },
  privacy: { enabled: true, retention_days: 0 },
  feedback: { sounds: true },
  updates: { enabled: true },
};

const builtin = (id: string, name: string, prompt = "", use_llm = true) => ({
  id,
  name,
  prompt,
  output_language: null as string | null,
  use_llm,
  builtin: true,
});

export const modes = {
  modes: [
    builtin("literal", "Literal", "", false),
    builtin("clean", "Limpio"),
    builtin(
      "email",
      "Email",
      "Da formato de correo electrónico: saludo, cuerpo en párrafos y despedida. Tono profesional y cordial. No inventes nombres: si no se menciona al destinatario, usa un saludo genérico.",
    ),
    builtin("message", "Mensaje"),
    builtin("prompt", "Prompt técnico"),
    { ...builtin("translate-en", "Traducir al inglés"), output_language: "inglés" },
    { ...builtin("tweets", "Tuits"), builtin: false },
  ],
  active: "clean",
};

export const history = [
  {
    id: 4,
    created_at: today(10, 42),
    raw_text: "",
    final_text:
      "Hola Marta, te escribo para confirmar la reunión del jueves a las diez. Llevaré el borrador del presupuesto para revisarlo juntos.",
    mode: "email",
    duration_ms: 12_800,
    stt_latency_ms: 700,
    llm_latency_ms: 500,
  },
  {
    id: 3,
    created_at: today(9, 15),
    raw_text: "",
    final_text:
      "Añade un test que compruebe que `run_pipeline` devuelve el texto bruto si Claude tarda más de 3 segundos.",
    mode: "prompt",
    duration_ms: 7_400,
    stt_latency_ms: 600,
    llm_latency_ms: 400,
  },
  {
    id: 2,
    created_at: today(8, 3),
    raw_text: "",
    final_text: "Llego en diez minutos, pide tú si quieres.",
    mode: "message",
    duration_ms: 2_100,
    stt_latency_ms: 400,
    llm_latency_ms: 300,
  },
  {
    id: 1,
    created_at: today(18, 30, 1),
    raw_text: "",
    final_text:
      "Ideas para el lanzamiento: probar la app en Mac, generar las claves del actualizador y publicar la primera versión.",
    mode: "clean",
    duration_ms: 9_000,
    stt_latency_ms: 600,
    llm_latency_ms: 400,
  },
];

export const vocabulary = {
  terms: ["Kubernetes", "Anthropic", "Tauri", "Groq"],
  replacements: [
    { from: "cloud", to: "Claude" },
    { from: "arroba empresa", to: "@empresa.com" },
  ],
};

export const keyHints = { groq: "gsk_••••7f2a", anthropic: null };

export const update = {
  configured: true,
  enabled: true,
  last_checked_ms: today(9, 0),
  available: { version: "0.2.0", notes: "[NOTA DE LA VERSIÓN 1]\n[NOTA DE LA VERSIÓN 2]" },
};

export const dictationResult = {
  text: "Hola, esto es una prueba. Mañana a las diez revisamos el informe con el equipo.",
  mode: "Limpio",
  audio_ms: 4_200,
  processing_ms: 1_100,
};
