# Changelog

## v0.1.2

### Dictados largos

- Grabaciones de hasta **3 horas** (antes 30 min). Se configura en Ajustes → Audio y transcripción → Duración máxima.
- El audio se transcribe **por trozos de 10 minutos mientras grabas**, con 5 s de solape para no cortar palabras. Al parar solo falta el último trozo, así que el texto llega casi al momento aunque hayas grabado horas. Cada trozo ocupa menos de los 25 MB que acepta Groq en el plan gratuito y el uso por hora queda por debajo de su límite.
- Las partes repetidas en el solape se eliminan al unir los trozos.
- Si Groq responde «demasiadas peticiones», la app espera lo que indique y reintenta ese trozo. Si un trozo falla del todo, se conservan el texto ya obtenido y el audio pendiente para reintentar desde el Historial.
- El overlay muestra el tiempo en h:mm:ss y cuántos trozos van transcritos.
- Ajustes avisa de que más de 2 h seguidas pueden agotar el límite de audio por hora de Groq (plan gratuito).

### Limpieza con Claude

- Ya no hay un tope fijo de 3 s: Claude tiene el tiempo que necesite según el largo del texto, y los dictados largos se limpian trozo a trozo mientras grabas.

### Correcciones

- «Instalar y reiniciar» no hacía nada en macOS: la confirmación usaba `window.confirm`, que el WebView de macOS no muestra (y devuelve «no»). Ahora la confirmación está dentro de la propia ventana y se muestra el progreso de la descarga. **Quien tenga la v0.1.0 o la v0.1.1 en macOS tiene que instalar esta versión a mano una vez**; las siguientes ya se podrán instalar desde la app.
- Los pasos de la instalación de actualizaciones y sus errores quedan en el log.

## v0.1.1

### Correcciones (macOS)

- macOS ya recuerda los permisos de micrófono y Accesibilidad: la app sale con firma ad-hoc completa y el entitlement de micrófono. Antes pedía el micrófono varias veces en cada dictado.
- Sin permiso de Accesibilidad, el texto dictado se queda en el portapapeles con un aviso que explica cómo activarlo. Antes se perdía (seguía en el Historial).
- El diálogo de permiso de Accesibilidad se abre como mucho una vez por sesión, no en cada dictado.
- El overlay dice «Copiado» cuando el texto se queda en el portapapeles en vez de pegarse.

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
