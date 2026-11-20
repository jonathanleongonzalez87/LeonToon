# Plan

1. [done] Preparar las herramientas para construir — Rust + MinGW instalados
2. [done] Dibujar el lienzo y las herramientas de dibujo
3. [done] Crear capas y línea de tiempo con fotogramas
4. [done] Añadir reproducción y papel cebolla
5. [done] Guardar, abrir y exportar imágenes
6. [done] Ronda 2: estabilidad — Decidido por mí (la persona delegó): deshacer ligero (Arc), guardado seguro con .bak y verificación, copia automática en segundo plano cada 30 s, aviso de recuperación tras cierre inesperado, captura de fallos internos con copia de emergencia y registro de errores, atajos bloqueados mientras se dibuja, flechas sin glifo sustituidas. 7 pruebas unitarias OK, build debug y release OK, capturas revisadas.
