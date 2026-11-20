# Cómo publicar una versión de LeonToon

Las versiones descargables se publican en **GitHub Releases**. Un workflow (`.github/workflows/release.yml`) compila `leontoon.exe` en modo release para Windows y crea la Release con el `.zip`.

## Publicar una versión

1. Comprueba que lo que va en la versión ya está en `main` (cada cambio entra con su Issue y su PR).
2. Sube el número de versión en `app/Cargo.toml` (`version = "0.2.0"`) con un Issue y un PR, y espera a que se fusione.
3. Crea y sube la etiqueta, con la `v` delante y el mismo número:

   ```powershell
   git checkout main
   git pull
   git tag v0.2.0
   git push origin v0.2.0
   ```

4. El workflow compila, empaqueta `LeonToon-v0.2.0-windows-x64.zip` y publica la Release con notas automáticas (los PR fusionados desde la versión anterior).

Si la etiqueta no coincide con la versión de `Cargo.toml`, el workflow falla antes de compilar. Las etiquetas con guion (`v0.2.0-beta1`) se publican como **pre-lanzamiento**.

## Probar sin publicar

En GitHub: **Actions → Release → Run workflow**. Con «Publicar» desmarcado, compila y deja el `.zip` como archivo de la ejecución (se descarga desde la página de la ejecución, sin crear ninguna Release).

## Notas

- El `.exe` no está firmado, así que Windows SmartScreen puede mostrar un aviso la primera vez.
- El icono del `.exe` lo incrusta `app/build.rs` con `windres`; si no está disponible, el programa se compila igual sin icono de archivo.
- Para exportar MP4 hace falta ffmpeg aparte.
