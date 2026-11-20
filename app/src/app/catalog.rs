//! Catálogo de la arquitectura de LeonToon (prototipo de interfaz v0.3 de la persona):
//! tipos de sala, conjuntos de salas por tipo de animación, herramientas por modo y editores de panel.

use super::*;

// ---------- Herramientas ----------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Tool {
    /// Modo objeto: mueve la capa (con claves). Modo edición: elige trazos.
    Select,
    RotateObj,
    Anchor,
    Camera,
    Brush,
    Vector,
    Pen,
    Text,
    Shape,
    Eraser,
    Fill,
    Picker,
    Hand,
    RotateView,
    Zoom,
}

/// Herramientas de cada modo, con separadores (None), en el orden del prototipo.
pub(crate) const TOOLS_OBJ: [Option<Tool>; 8] = [
    Some(Tool::Select),
    Some(Tool::RotateObj),
    Some(Tool::Anchor),
    Some(Tool::Camera),
    None,
    Some(Tool::Hand),
    Some(Tool::RotateView),
    Some(Tool::Zoom),
];
pub(crate) const TOOLS_EDIT: [Option<Tool>; 14] = [
    Some(Tool::Select),
    None,
    Some(Tool::Brush),
    Some(Tool::Vector),
    Some(Tool::Pen),
    Some(Tool::Text),
    Some(Tool::Shape),
    Some(Tool::Eraser),
    Some(Tool::Fill),
    Some(Tool::Picker),
    None,
    Some(Tool::Hand),
    Some(Tool::RotateView),
    Some(Tool::Zoom),
];

impl Tool {
    pub(crate) const ALL: [Tool; 15] = [
        Tool::Select,
        Tool::RotateObj,
        Tool::Anchor,
        Tool::Camera,
        Tool::Brush,
        Tool::Vector,
        Tool::Pen,
        Tool::Text,
        Tool::Shape,
        Tool::Eraser,
        Tool::Fill,
        Tool::Picker,
        Tool::Hand,
        Tool::RotateView,
        Tool::Zoom,
    ];
    pub(crate) fn name(self) -> &'static str {
        match self {
            Tool::Select => "Seleccionar",
            Tool::RotateObj => "Rotación",
            Tool::Anchor => "Punto de anclaje",
            Tool::Camera => "Cámara unificada",
            Tool::Brush => "Pincel",
            Tool::Vector => "Lápiz vectorial",
            Tool::Pen => "Pluma vectorial",
            Tool::Text => "Texto",
            Tool::Shape => "Formas",
            Tool::Eraser => "Borrador",
            Tool::Fill => "Relleno",
            Tool::Picker => "Cuentagotas",
            Tool::Hand => "Mano",
            Tool::RotateView => "Rotar vista",
            Tool::Zoom => "Zoom",
        }
    }
    /// Atajo (letra sola, como en el prototipo; Texto usa Ctrl+T).
    pub(crate) fn key(self) -> Option<Key> {
        Some(match self {
            Tool::Select => Key::V,
            Tool::RotateObj => Key::W,
            Tool::Anchor => Key::Y,
            Tool::Camera => Key::C,
            Tool::Brush => Key::B,
            Tool::Vector => Key::L,
            Tool::Pen => Key::P,
            Tool::Shape => Key::Q,
            Tool::Eraser => Key::E,
            Tool::Fill => Key::G,
            Tool::Picker => Key::I,
            Tool::Hand => Key::H,
            Tool::RotateView => Key::R,
            Tool::Zoom => Key::Z,
            Tool::Text => return None,
        })
    }
    pub(crate) fn letter(self) -> &'static str {
        match self {
            Tool::Select => "V",
            Tool::RotateObj => "W",
            Tool::Anchor => "Y",
            Tool::Camera => "C",
            Tool::Brush => "B",
            Tool::Vector => "L",
            Tool::Pen => "P",
            Tool::Text => "Ctrl+T",
            Tool::Shape => "Q",
            Tool::Eraser => "E",
            Tool::Fill => "G",
            Tool::Picker => "I",
            Tool::Hand => "H",
            Tool::RotateView => "R",
            Tool::Zoom => "Z",
        }
    }
    pub(crate) fn tip(self) -> &'static str {
        match self {
            Tool::Select => "Modo objeto: selecciona y mueve capas (crea claves; Mayús gira, Ctrl escala). Modo edición: selecciona trazos.",
            Tool::RotateObj => "Gira la capa alrededor de su punto de anclaje (crea una clave).",
            Tool::Anchor => "Mueve el punto de anclaje sin mover la capa. Pendiente.",
            Tool::Camera => "Orbita, desplaza y acerca la cámara activa. Pendiente.",
            Tool::Brush => "Dibujo a mano alzada, con suavizado.",
            Tool::Vector => "Trazos finos a mano alzada, editables, ideales para la limpieza.",
            Tool::Pen => "Curvas Bézier por puntos y tiradores. Pendiente.",
            Tool::Text => "Crea o edita una capa de texto. Pendiente.",
            Tool::Shape => "Líneas, rectángulos y elipses (elige la forma en la cabecera). Mayús: 45° o proporción fija.",
            Tool::Eraser => "Borra trazos; las figuras rellenas se borran enteras.",
            Tool::Fill => "Rellena áreas cerradas con el color de la paleta.",
            Tool::Picker => "Toma un color del dibujo.",
            Tool::Hand => "Desplaza la vista. También con el clic central o Espacio + arrastrar.",
            Tool::RotateView => "Gira la mesa de luz como una hoja real. Pendiente.",
            Tool::Zoom => "Clic: acercar. Clic derecho o Alt + clic: alejar. También con la rueda.",
        }
    }
    /// Herramientas que aún no hacen nada (se ven en la barra para mostrar el plan).
    pub(crate) fn pending(self) -> bool {
        matches!(self, Tool::Anchor | Tool::Camera | Tool::Pen | Tool::Text | Tool::RotateView)
    }
    /// Herramientas que solo miran (no cambian la capa).
    pub(crate) fn is_view(self) -> bool {
        matches!(self, Tool::Zoom | Tool::Hand | Tool::Picker | Tool::RotateView)
    }
    pub(crate) fn edit_only(self) -> bool {
        matches!(self, Tool::Brush | Tool::Vector | Tool::Pen | Tool::Text | Tool::Shape | Tool::Eraser | Tool::Fill | Tool::Picker)
    }
    pub(crate) fn obj_only(self) -> bool {
        matches!(self, Tool::RotateObj | Tool::Anchor | Tool::Camera)
    }
    /// Herramientas que pintan trazos (muestran grosor, suavizado y opacidad).
    pub(crate) fn draws(self) -> bool {
        matches!(self, Tool::Brush | Tool::Vector | Tool::Pen | Tool::Eraser | Tool::Shape)
    }
    /// El motor de dibujo lo decide la capa activa, nunca se mezclan: el Lápiz y la Pluma vectoriales solo dibujan en
    /// capas vectoriales (no convierten trazos en píxeles) y el Pincel de píxeles solo actúa en capas de mapa de bits.
    /// Devuelve el motivo si la herramienta no se puede usar en esa clase de capa.
    pub(crate) fn layer_block(self, kind: LayerKind) -> Option<&'static str> {
        let paints = self.draws() || matches!(self, Tool::Fill);
        if paints && matches!(kind, LayerKind::Camera | LayerKind::Light | LayerKind::Null | LayerKind::Text) {
            return Some("Esta capa no contiene dibujos: elige una capa vectorial o de mapa de bits.");
        }
        if matches!(self, Tool::Vector | Tool::Pen) && kind == LayerKind::Bitmap {
            return Some("El lápiz y la pluma vectoriales solo dibujan en capas vectoriales y esta es una capa de mapa de bits: usa el Pincel, o elige una capa vectorial.");
        }
        None
    }
}

// ---------- Salas ----------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum RoomKind {
    Layout,
    Dibujo,
    Limpieza,
    Animacion,
    Timing,
    Interp,
    Rig,
    Comp,
    Color,
    Story,
    Animatic,
    Exportar,
}

/// Qué muestra el panel de propiedades de cada sala.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum RightKind {
    Color,
    Interp,
    Rig,
    Comp,
    Palettes,
    StoryPdf,
    Animatic,
    Export,
}

/// Distribución de una sala (ROOMT del prototipo).
pub(crate) struct RoomDef {
    pub id: &'static str,
    pub name: &'static str,
    /// true = línea de tiempo abajo; false = hoja de exposición al lado de la mesa de luz.
    pub tl_horizontal: bool,
    /// Alto de la línea de tiempo en px (cuando va abajo).
    pub tl_h: f32,
    pub right: RightKind,
    pub tool: Option<Tool>,
    /// El centro es el StoryBoard en lugar de la mesa de luz.
    pub story: bool,
    pub desc: &'static str,
}

impl RoomKind {
    pub(crate) const ALL: [RoomKind; 12] = [
        RoomKind::Layout,
        RoomKind::Dibujo,
        RoomKind::Limpieza,
        RoomKind::Animacion,
        RoomKind::Timing,
        RoomKind::Interp,
        RoomKind::Rig,
        RoomKind::Comp,
        RoomKind::Color,
        RoomKind::Story,
        RoomKind::Animatic,
        RoomKind::Exportar,
    ];
    pub(crate) fn def(self) -> RoomDef {
        let d = |id, name, tl_horizontal, tl_h, right, tool, story, desc| RoomDef {
            id,
            name,
            tl_horizontal,
            tl_h,
            right,
            tool,
            story,
            desc,
        };
        use RightKind as R;
        match self {
            RoomKind::Layout => d("basic", "Layout", true, 210.0, R::Color, None, false, "Uso general, como el Layout de Blender"),
            RoomKind::Dibujo => d("dibujo", "Dibujo", true, 170.0, R::Color, Some(Tool::Brush), false, "Mesa de luz amplia para dibujar"),
            RoomKind::Limpieza => d("limpieza", "Limpieza", true, 170.0, R::Color, Some(Tool::Vector), false, "Línea final vectorial sobre el boceto"),
            RoomKind::Animacion => d("animacion", "Animación", false, 0.0, R::Color, None, false, "Hoja de exposición y mesa de luz lado a lado"),
            RoomKind::Timing => d("timing", "Timing", false, 0.0, R::Color, None, false, "Ajuste de exposiciones y ritmo"),
            RoomKind::Interp => d("interp", "Interpolación", true, 290.0, R::Interp, None, false, "Animación por claves y curvas"),
            RoomKind::Rig => d("rig", "Rigging", true, 220.0, R::Rig, None, false, "Esqueletos y piezas para cutout"),
            RoomKind::Comp => d("comp", "Composición 3D", true, 190.0, R::Comp, None, false, "Planos BG, MG y FG en profundidad"),
            RoomKind::Color => d("color", "Color", true, 160.0, R::Palettes, Some(Tool::Fill), false, "Paletas y relleno"),
            RoomKind::Story => d("story", "Storyboard", true, 0.0, R::StoryPdf, None, true, "Planos con textos y exportación a PDF"),
            RoomKind::Animatic => d("animatic", "Animatic", true, 230.0, R::Animatic, None, false, "Planos en el tiempo con duración y transiciones"),
            RoomKind::Exportar => d("exportar", "Exportar", true, 150.0, R::Export, None, false, "PNG, hoja de sprites, GIF y MP4"),
        }
    }
    pub(crate) fn from_id(id: &str) -> Option<RoomKind> {
        RoomKind::ALL.into_iter().find(|r| r.def().id == id)
    }
}

/// Grupos del menú «+ Nueva sala» (SUBS.NEWROOM del prototipo).
pub(crate) const ROOM_GROUPS: [(&str, &[RoomKind]); 4] = [
    ("Producción", &[RoomKind::Story, RoomKind::Animatic]),
    ("Dibujo", &[RoomKind::Layout, RoomKind::Dibujo, RoomKind::Limpieza, RoomKind::Color]),
    ("Animación", &[RoomKind::Animacion, RoomKind::Timing, RoomKind::Interp, RoomKind::Rig]),
    ("Composición y salida", &[RoomKind::Comp, RoomKind::Exportar]),
];

/// Conjunto de salas por tipo de animación (ANIMT del prototipo).
pub(crate) struct RoomSet {
    pub id: &'static str,
    pub name: &'static str,
    pub rooms: &'static [RoomKind],
}

pub(crate) const ROOM_SETS: [RoomSet; 5] = [
    RoomSet {
        id: "trad",
        name: "Animación tradicional",
        rooms: &[
            RoomKind::Layout,
            RoomKind::Dibujo,
            RoomKind::Limpieza,
            RoomKind::Animacion,
            RoomKind::Timing,
            RoomKind::Color,
            RoomKind::Exportar,
        ],
    },
    RoomSet {
        id: "cutout",
        name: "Cutout o rigging 2D",
        rooms: &[RoomKind::Layout, RoomKind::Rig, RoomKind::Animacion, RoomKind::Interp, RoomKind::Exportar],
    },
    RoomSet { id: "mg", name: "Motion graphics", rooms: &[RoomKind::Layout, RoomKind::Interp, RoomKind::Comp, RoomKind::Exportar] },
    RoomSet { id: "story", name: "Storyboard", rooms: &[RoomKind::Story, RoomKind::Exportar] },
    RoomSet { id: "animatic", name: "Animatic", rooms: &[RoomKind::Story, RoomKind::Animatic, RoomKind::Exportar] },
];

pub(crate) fn room_set(id: &str) -> &'static RoomSet {
    ROOM_SETS.iter().find(|s| s.id == id).unwrap_or(&ROOM_SETS[0])
}

// ---------- Editores de panel (el tipo de editor se cambia desde su cabecera) ----------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Editor {
    /// Lo que pide la sala (Color, Interpolación, Exportar…).
    RoomProps,
    Palettes,
    Curves,
    Explorer,
    Schematic,
    Substitution,
    Preview,
}

impl Editor {
    pub(crate) const ALL: [Editor; 7] =
        [Editor::RoomProps, Editor::Palettes, Editor::Curves, Editor::Explorer, Editor::Schematic, Editor::Substitution, Editor::Preview];
    pub(crate) fn name(self) -> &'static str {
        match self {
            Editor::RoomProps => "Propiedades",
            Editor::Palettes => "Biblioteca de paletas",
            Editor::Curves => "Editor de curvas",
            Editor::Explorer => "Explorador del proyecto",
            Editor::Schematic => "Esquema de capas",
            Editor::Substitution => "Sustitución de dibujo",
            Editor::Preview => "Vista previa",
        }
    }
}

impl RightKind {
    pub(crate) fn title(self) -> &'static str {
        match self {
            RightKind::Color => "Paleta",
            RightKind::Interp => "Interpolación",
            RightKind::Rig => "Rigging",
            RightKind::Comp => "Composición",
            RightKind::Palettes => "Paletas",
            RightKind::StoryPdf => "Plano",
            RightKind::Animatic => "Animatic",
            RightKind::Export => "Exportar",
        }
    }
}
