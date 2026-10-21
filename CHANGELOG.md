# Changelog

## v0.1.0 — primera versión

Dictado por voz en cualquier aplicación: pulsas el atajo, hablas, lo vuelves a pulsar y el texto transcrito y pulido aparece donde está el cursor.

### Dictado

- Atajo global configurable: `Option+Space` en macOS y `Ctrl+Alt+Space` en Windows y Linux. Se pulsa una vez para empezar y otra para terminar, o se mantiene pulsado. `Esc` cancela.
- Transcripción con Groq Whisper (`whisper-large-v3-turbo`), con idioma automático o fijo.
- Limpieza y formato con Claude Haiku 5.5. Si Claude falla o tarda más de 3 s, se pega el texto sin limpiar.
- El texto se pega en la aplicación activa, o solo se copia al portapapeles.
- Overlay con cuatro estados: grabando, procesando, pegado y error con reintento.

### Personalización

- Modos: Literal, Limpio, Email, Mensaje, Prompt técnico, Traducir al inglés y los tuyos. Se cambian con `Ctrl+Option+M` / `Ctrl+Alt+M` o desde la bandeja.
- Vocabulario: términos que siempre se escriben bien y reemplazos del tipo "si oyes → escribe".
- Historial local buscable, con copiar, pegar de nuevo, borrar y retención configurable.

### App

- Vive en la bandeja del sistema, con el estado y el modo activo. Puede iniciarse con el sistema.
- Onboarding de 5 pasos: keys de Groq y Anthropic, permisos de macOS y un dictado de prueba.
- Las API keys se guardan en el llavero del sistema y nunca en texto plano. La interfaz solo muestra el prefijo y los últimos 4 caracteres.
- Errores con notificación nativa y sonidos opcionales. Si un servicio falla, el audio se conserva para reintentar.
- Logs rotativos sin keys ni texto dictado.
- Actualizaciones automáticas firmadas: comprueba al iniciar y una vez al día, y solo instala con tu confirmación.

### Limitaciones conocidas

- Los instaladores no están firmados con certificados de Apple ni de Microsoft. Gatekeeper (macOS) y SmartScreen (Windows) avisarán la primera vez.
- Los builds de macOS y Linux salen del workflow de release; solo se han probado a mano en macOS.
