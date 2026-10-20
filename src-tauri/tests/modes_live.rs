//! Evaluación en vivo de los modos predefinidos con comprobaciones deterministas.
//! `LIVE_API_TESTS=1 cargo test --test modes_live -- --ignored --nocapture`

use regex::Regex;
use voice_recorder_lib::llm::{AnthropicCleaner, TextCleaner, clean_or_raw};
use voice_recorder_lib::modes::ModesState;

struct Case {
    mode: &'static str,
    input: &'static str,
    check: fn(&str, &str) -> Result<(), String>,
}

fn ok(cond: bool, why: &str) -> Result<(), String> {
    if cond { Ok(()) } else { Err(why.into()) }
}

fn lang(text: &str) -> &'static str {
    whatlang::detect_lang(text).map(|l| l.code()).unwrap_or("?")
}

const CASES: &[Case] = &[
    Case {
        mode: "translate-en",
        input: "mañana te envío el presupuesto actualizado con los nuevos precios",
        check: |_, out| ok(lang(out) == "eng", "la salida no está en inglés"),
    },
    Case {
        mode: "email",
        input: "hola Carlos te escribo para confirmar la reunión del jueves a las diez en la sala dos gracias",
        check: |_, out| {
            let greet = Regex::new(r"(?i)^\s*(hola|estimad[oa]|buen[oa]s|querid[oa])").unwrap();
            let close = Regex::new(
                r"(?i)(saludos|un saludo|atentamente|gracias|cordialmente)[^\n]*\s*(\n.*)?$",
            )
            .unwrap();
            ok(greet.is_match(out), "sin saludo")?;
            ok(close.is_match(out), "sin despedida")?;
            ok(out.contains('\n'), "sin párrafos")
        },
    },
    Case {
        mode: "message",
        input: "oye ya llegué al restaurante te espero en la mesa del fondo",
        check: |input, out| {
            ok(
                !Regex::new(r"(?i)(estimad|atentamente|cordialmente)")
                    .unwrap()
                    .is_match(out),
                "tono formal",
            )?;
            ok(
                out.chars().count() as f64 <= input.chars().count() as f64 * 1.3,
                "demasiado largo",
            )
        },
    },
    Case {
        mode: "prompt",
        input: "eh por favor podrías crear una función en python que se llame parse dates que reciba una lista de strings y devuelva objetos datetime gracias",
        check: |_, out| {
            ok(
                !Regex::new(r"(?i)(por favor|gracias|podrías|hola)")
                    .unwrap()
                    .is_match(out),
                "relleno conversacional",
            )?;
            ok(
                Regex::new(r"(?i)parse.?dates").unwrap().is_match(out),
                "perdió el nombre de la función",
            )?;
            ok(
                Regex::new(r"(?i)python").unwrap().is_match(out),
                "perdió el lenguaje",
            )?;
            ok(
                Regex::new(r"(?i)datetime").unwrap().is_match(out),
                "perdió datetime",
            )
        },
    },
    Case {
        mode: "clean",
        input: "eh bueno o sea el proyecto este va bien y eh lo entregamos el lunes",
        check: |_, out| {
            ok(
                !Regex::new(r"(?i)\b(eh|o sea)\b").unwrap().is_match(out),
                "quedaron muletillas",
            )?;
            ok(lang(out) == "spa", "cambió el idioma")
        },
    },
    Case {
        mode: "literal",
        input: "eh esto se pega tal cual",
        check: |input, out| ok(out == input, "Literal modificó el texto"),
    },
];

#[test]
#[ignore = "llama a la API de pago de Anthropic"]
fn builtin_modes_pass_at_least_5_of_6() {
    if std::env::var("LIVE_API_TESTS").as_deref() != Ok("1") {
        eprintln!("omitido: LIVE_API_TESTS != 1");
        return;
    }
    let key = std::env::var("ANTHROPIC_API_KEY").expect("ANTHROPIC_API_KEY");
    let cleaner = AnthropicCleaner::with_base_url(
        "https://api.anthropic.com/v1",
        key,
        std::time::Duration::from_secs(30),
    );
    let modes = ModesState::initial();
    let mut passed = 0;
    for case in CASES {
        let mode = modes.get(case.mode).unwrap();
        // Igual que el pipeline: Literal no pasa por Claude.
        let c = mode.wants_llm(true).then_some(&cleaner as &dyn TextCleaner);
        let out = clean_or_raw(c, case.input, &mode.instructions());
        let verdict = (case.check)(case.input, out.text());
        if verdict.is_ok() {
            passed += 1;
        }
        println!(
            "[{}] {} → «{}» {}",
            if verdict.is_ok() { "OK" } else { "FALLA" },
            case.mode,
            out.text().replace('\n', "⏎"),
            verdict.err().unwrap_or_default()
        );
    }
    println!("LIVE modes: {passed}/{}", CASES.len());
    assert!(passed >= 5, "solo {passed}/{} modos pasaron", CASES.len());
}
