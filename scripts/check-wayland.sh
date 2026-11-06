#!/usr/bin/env bash
# Issue #38: smoke test del backend de atajos en Linux Wayland.
# Lanza el binario, espera a que el portal GlobalShortcuts vincule los atajos
# y falla si no aparece `status=bound` en app.log (o aparece `status=error`).
#
# Uso: scripts/check-wayland.sh [ruta-al-binario] [segundos]
#   binario por defecto: src-tauri/target/debug/voice-recorder
#   segundos por defecto: 60 (la primera vez GNOME pide confirmar el atajo)
set -u

BIN="${1:-src-tauri/target/debug/voice-recorder}"
TIMEOUT="${2:-60}"
LOG="${XDG_DATA_HOME:-$HOME/.local/share}/com.alexander940.voicerecorder/logs/app.log"

if [[ -z "${WAYLAND_DISPLAY:-}" && "${XDG_SESSION_TYPE:-}" != "wayland" ]]; then
  echo "✗ esta sesión no es Wayland (WAYLAND_DISPLAY vacío)"; exit 2
fi
if [[ ! -x "$BIN" ]]; then
  echo "✗ no existe el binario $BIN (compila con: npm run tauri build -- --debug --no-bundle)"; exit 2
fi
if pgrep -x voice-recorder >/dev/null; then
  echo "✗ ya hay un voice-recorder en marcha; ciérralo antes"; exit 2
fi

mkdir -p "$(dirname "$LOG")"; touch "$LOG"
START=$(wc -l < "$LOG")

"$BIN" >/dev/null 2>&1 &
PID=$!
trap 'kill "$PID" 2>/dev/null; wait "$PID" 2>/dev/null' EXIT

new_lines() { tail -n +"$((START + 1))" "$LOG"; }

for ((i = 0; i < TIMEOUT; i++)); do
  if new_lines | grep -q 'hotkey: backend=portal status=error'; then
    echo "✗ el portal devolvió un error:"; new_lines | grep 'hotkey:'; exit 1
  fi
  if new_lines | grep -q 'hotkey: backend=portal status=bound id=dictate'; then
    echo "✓ atajos vinculados por el portal:"; new_lines | grep 'hotkey:'; exit 0
  fi
  if ! kill -0 "$PID" 2>/dev/null; then
    echo "✗ la app terminó antes de vincular los atajos:"; new_lines | tail -20; exit 1
  fi
  sleep 1
done

echo "✗ pasaron $TIMEOUT s sin 'status=bound' (¿hay un diálogo de GNOME esperando?):"
new_lines | grep 'hotkey:' || new_lines | tail -20
exit 1
