# Integración de Kitty Terminal con Material You & Noctalia

Este documento detalla la arquitectura, especificaciones técnicas y configuración para la sincronización dinámica de paletas de colores en el emulador de terminal **Kitty**, integrando el generador **`kde-material-you-colors`** (KDE Plasma) y el ecosistema **Noctalia / KineticWE** con aislamiento mutuo y recarga en caliente.

---

## 1. Objetivos de Diseño

1. **Sincronización de Paleta sin Acoplamiento Rígido:**
   * La terminal recibe la paleta cromática calculada dinámicamente a partir del wallpaper o color base (Material 3).
   * Kitty se mantiene de forma predeterminada en **modo oscuro** (`material-you-dark.conf`), pero también se genera la variante clara (`material-you-light.conf`).
2. **Compatibilidad con `kitten themes`:**
   * Los archivos se depositan en `~/.config/kitty/themes/` con cabeceras de metadatos estándar (`## name: Material You Dark` y `## name: Material You Light`).
   * Kitty los cataloga automáticamente bajo la pestaña `User #1` / Temas de usuario.
3. **Convivencia y Conmutación Plasma vs KineticWE / Noctalia:**
   * Se elimina la necesidad de editar `~/.config/kitty/kitty.conf` al cambiar de sesión o entorno de escritorio.
   * Se utiliza el patrón de **Symlink Dinámico (`current-theme.conf`)**.
4. **Recarga en Caliente (Zero Disruption):**
   * Emisión de la señal POSIX `SIGUSR1` a los procesos activos de Kitty para actualizar colores instantáneamente sin cerrar sesiones activas.

---

## 2. Arquitectura de Conmutación Dinámica

### Patrón de Enlace Simbólico (`current-theme.conf`)

En lugar de que cada aplicación sobreescriba `kitty.conf`, la configuración de Kitty delega el tema a un único archivo incluido:

```text
~/.config/kitty/
├── kitty.conf              <-- Contiene: "include current-theme.conf"
├── current-theme.conf      <-- SYMLINK DINÁMICO
└── themes/
    ├── material-you-dark.conf    <-- Generado por kde-material-you-colors
    ├── material-you-light.conf   <-- Generado por kde-material-you-colors
    └── noctalia.conf             <-- Administrado por Noctalia
```

### Comportamiento por Entorno:

* **En KDE Plasma (con `kde-material-you-colors`):**
  1. Extrae los colores del fondo de pantalla mediante M3 / Celebi.
  2. Escribe `~/.config/kitty/themes/material-you-dark.conf` y `~/.config/kitty/themes/material-you-light.conf`.
  3. Crea o actualiza el enlace simbólico relativo:
     ```bash
     ln -sfn themes/material-you-dark.conf ~/.config/kitty/current-theme.conf
     ```
  4. Envía `pkill -USR1 -x kitty` para aplicar la paleta inmediatamente.

* **En KineticWE / Noctalia:**
  1. Noctalia genera su paleta en `~/.config/kitty/themes/noctalia.conf`.
  2. Su hook de cambio conmuta el enlace simbólico:
     ```bash
     ln -sfn themes/noctalia.conf ~/.config/kitty/current-theme.conf
     ```
  3. Envía `pkill -USR1 -x kitty`.

* **Ventaja:**
  Ambos entornos operan sobre sus propios archivos de tema y solo compiten por el enlace `current-theme.conf`. Ningún entorno rompe o sobreescribe las directivas personalizadas del usuario en `kitty.conf`.

---

## 3. Especificación del Formato de Tema para Kitty

Los archivos generados cumplen la especificación de temas de Kitty:

```conf
## name: Material You Dark
## author: kde-material-you-colors
## upstream: https://github.com/luisbocanegra/kde-material-you-colors

# 16-color palette
color0  #1a1c1e
color1  #ba1a1a
color2  #386a20
color3  #715b00
color4  #006495
color5  #805158
color6  #006877
color7  #e2e2e5
color8  #43474e
color9  #ff5449
color10 #528637
color11 #8d7318
color12 #2c7eb4
color13 #9c6970
color14 #258190
color15 #fdfcff

# Special colors
background            #1a1c1e
foreground            #e2e2e5
cursor                #82d3e0
cursor_text_color     #1a1c1e
selection_foreground  #1a1c1e
selection_background  #82d3e0

# Window borders
active_border_color   #82d3e0
inactive_border_color #2d3135
url_color             #006495

# Tab bar
active_tab_foreground   #1a1c1e
active_tab_background   #82d3e0
inactive_tab_foreground #e2e2e5
inactive_tab_background #2d3135

# Cursor trail
cursor_trail_color      #e2e2e5
```

---

## 4. Opciones de Configuración en `kde-material-you-colors`

### 4.1. Archivo de Configuración (`~/.config/kde-material-you-colors/config.conf`)
Bajo la sección `[CUSTOM]`:
```ini
[CUSTOM]
kitty = true
```

### 4.2. Línea de Comandos (CLI)
```bash
# Exportar tema de Kitty junto con los demás objetivos
kde-material-you-colors --kitty

# Combinado con extracción de wallpaper o color específico
kde-material-you-colors --kitty --color "#389F86"
```

### 4.3. Interfaz Gráfica (GUI)
En la ventana de configuración de `kde-material-you-colors`, dentro de la sección **Theming targets & exports**, se dispone de la casilla:
* `[x] Export Kitty theme files`

Al marcarla y presionar **Save and Apply**, se activa la exportación y actualización periódica o por eventos de fondo de pantalla.

---

## 5. Puesta en Marcha (Setup Inicial)

### Paso 1: Configurar `kitty.conf`
Asegurarse de que `~/.config/kitty/kitty.conf` incluya `current-theme.conf` al final:

```conf
# Configuración general de Kitty
cursor_trail 200
cursor_trail_decay 0.1 0.4
cursor_trail_start_threshold 2
tab_bar_min_tabs 1

# Incluir tema dinámico activo
include current-theme.conf
```

### Paso 2: Activar la opción en `kde-material-you-colors`
Ejecutar:
```bash
kde-material-you-colors --kitty
```
O activar la casilla en la GUI.

### Paso 3: Verificar en Kitty
1. Comprobar que existe el symlink:
   ```bash
   ls -l ~/.config/kitty/current-theme.conf
   # Salida esperada: ... current-theme.conf -> themes/material-you-dark.conf
   ```
2. Abrir el selector interactivo de Kitty:
   ```bash
   kitten themes
   ```
   En la lista aparecerán `Material You Dark` y `Material You Light`.
