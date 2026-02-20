---
trigger: always_on
---

1) Principios generales

Actúa como operador DevOps: cada cambio debe quedar versionado, desplegado y verificado.

No inventes resultados. Comprueba con comandos y reporta lo observado (logs, status, puertos, endpoints).

Mantén el sistema funcionando al final: si algo falla, prioriza rollback o corrección inmediata antes de “dar por terminado”.

2) Seguridad mínima obligatoria

Nunca pegues en el chat contenido de llaves privadas, tokens, .env, credenciales, o salidas que los expongan.

Si necesitas mostrar output, redacta/oculta secretos (por ejemplo, reemplaza con ***).

Si un comando puede ser destructivo (borrar, reset, drop DB), confirma que existe respaldo o usa alternativas seguras.

3) Conexión al VPS

Para conectarte al VPS usa este comando (sin modificarlo):

ssh -i C:\Users\matid\.ssh\polybot_ed25519 -o StrictHostKeyChecking=no root@178.128.224.8

Una vez dentro:

Identifica el proyecto (ruta del repo y servicio).

Verifica estado base antes de tocar nada:

uptime / espacio: uptime, df -h

servicio: systemctl status polybot (o el nombre real)

logs recientes: journalctl -u polybot -n 100 --no-pager (o equivalente)

Nunca hagas npm o pnpm run build en el VPS ya que el dashboard esta en Vercel hosteado no en el VPS

El build debe ser siempre "cargo build --release --features dashboard"

4) Flujo obligatorio de cambios (GitHub como fuente de verdad)

Todo cambio debe seguir este orden:

Modificar código (local o en el VPS, según tu operación).

Commit + push a GitHub con mensajes claros y pequeños.

En el VPS: git pull para traer la actualización.

Build del servicio Polybot incluyendo el dashboard (si existe un flag/comando, debe usarse siempre).

Restart/Reload del servicio.

Verificación final (health check + logs + endpoint).

Reglas de commits:

Commits pequeños, coherentes y fáciles de revertir.

Mensaje tipo: fix: ..., feat: ..., chore: ...

Evita mezclar refactors enormes con cambios funcionales.

5) Procedimiento estándar en el VPS (Deploy)

En el VPS, antes de actualizar:

Entra a la carpeta del repo.

Revisa estado: git status, git log -1 --oneline

Si hay cambios locales sin commitear, no los pierdas: commitea o guarda con stash con nota.

Actualización:

git pull

Reinicio:

Reinicia con el gestor correcto (preferir systemd):

systemctl restart polybot

systemctl status polybot --no-pager

6) Verificación obligatoria post-deploy

Después del restart:

Verifica logs sin errores:

journalctl -u polybot -n 200 --no-pager

Verifica proceso/puerto:

ss -lntp o lsof -i :PUERTO

Verifica endpoint/health:

curl -f http://localhost:8088/health (o la ruta real)

Si algo falla:

No lo “maquilles”. Reporta el error principal y aplica corrección.

Si el arreglo toma mucho, haz rollback a un commit anterior estable y deja el servicio arriba.

7) Manejo de errores y rollback

Si el servicio no levanta tras el deploy:

Revisa logs y causa raíz (dependencias, env vars, build roto, permisos).

Aplica fix rápido y vuelve a desplegar.

Si no es rápido: vuelve al último commit estable:

git checkout <commit_estable>

rebuild + restart + verificación

Documenta qué se rompió y qué se hizo.

8) Estándares de “entrega”

Considera la tarea terminada solo si:

El cambio está en GitHub (push realizado).

El VPS tiene el último código (git log -1 coincide).

Polybot está corriendo y el dashboard incluido en el build.

Health check y logs están OK.