//! Every command the user can run, and the tools.

/// A command. Menus, shortcuts, context menus and the command palette all
/// run the same actions, so behaviour never depends on how it was invoked.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Action {
    // File
    New,
    Open,
    Save,
    SaveAs,
    Quit,
    // Edit
    Undo,
    Redo,
    Cut,
    Copy,
    Paste,
    Duplicate,
    Delete,
    SelectAll,
    SelectNone,
    Rename,
    Preferences,
    CommandPalette,
    // Layer
    NewVectorLayer,
    NewGroupLayer,
    NewBoneLayer,
    GroupLayers,
    Ungroup,
    MoveLayerUp,
    MoveLayerDown,
    ToggleVisibility,
    LayerProperties,
    // Animation
    PlayPause,
    NextFrame,
    PrevFrame,
    FirstFrame,
    LastFrame,
    NextKey,
    PrevKey,
    AddKey,
    DeleteKey,
    // View
    ZoomIn,
    ZoomOut,
    FitView,
    ActualSize,
    // Tools
    Tool(Tool),
    /// Moho's `A`: Add Point on a vector layer, Add Bone on a bone layer.
    AddPointOrBone,
}

impl Action {
    pub fn label(self) -> &'static str {
        use Action::*;
        match self {
            New => "New",
            Open => "Open…",
            Save => "Save",
            SaveAs => "Save As…",
            Quit => "Quit",
            Undo => "Undo",
            Redo => "Redo",
            Cut => "Cut",
            Copy => "Copy",
            Paste => "Paste",
            Duplicate => "Duplicate",
            Delete => "Delete",
            SelectAll => "Select All Layers",
            SelectNone => "Select None",
            Rename => "Rename Layer",
            Preferences => "Preferences…",
            CommandPalette => "Command Palette…",
            NewVectorLayer => "New Vector Layer",
            NewGroupLayer => "New Group",
            NewBoneLayer => "New Bone Layer",
            GroupLayers => "Group Layers",
            Ungroup => "Ungroup",
            MoveLayerUp => "Move Layer Up",
            MoveLayerDown => "Move Layer Down",
            ToggleVisibility => "Show/Hide Layer",
            LayerProperties => "Layer Properties",
            PlayPause => "Play/Pause",
            NextFrame => "Next Frame",
            PrevFrame => "Previous Frame",
            FirstFrame => "First Frame",
            LastFrame => "Last Frame",
            NextKey => "Next Keyframe",
            PrevKey => "Previous Keyframe",
            AddKey => "Add Keyframe",
            DeleteKey => "Delete Keyframe",
            ZoomIn => "Zoom In",
            ZoomOut => "Zoom Out",
            FitView => "Fit Canvas",
            ActualSize => "Actual Size (100%)",
            Tool(t) => t.label(),
            AddPointOrBone => "Add Point / Add Bone",
        }
    }

    /// Every action, for the command palette.
    pub fn all() -> Vec<Action> {
        use Action::*;
        let mut v = vec![
            New,
            Open,
            Save,
            SaveAs,
            Quit,
            Undo,
            Redo,
            Cut,
            Copy,
            Paste,
            Duplicate,
            Delete,
            SelectAll,
            SelectNone,
            Rename,
            Preferences,
            CommandPalette,
            NewVectorLayer,
            NewGroupLayer,
            NewBoneLayer,
            GroupLayers,
            Ungroup,
            MoveLayerUp,
            MoveLayerDown,
            ToggleVisibility,
            LayerProperties,
            PlayPause,
            NextFrame,
            PrevFrame,
            FirstFrame,
            LastFrame,
            NextKey,
            PrevKey,
            AddKey,
            DeleteKey,
            ZoomIn,
            ZoomOut,
            FitView,
            ActualSize,
        ];
        v.extend(crate::action::Tool::ALL.iter().map(|t| Tool(*t)));
        v
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ToolGroup {
    Draw,
    Fill,
    Layer,
    Bone,
    Workspace,
}

impl ToolGroup {
    pub fn label(self) -> &'static str {
        match self {
            Self::Draw => "Draw",
            Self::Fill => "Fill",
            Self::Layer => "Layer",
            Self::Bone => "Bone",
            Self::Workspace => "Workspace",
        }
    }

    pub const ALL: [ToolGroup; 5] = [Self::Draw, Self::Fill, Self::Layer, Self::Bone, Self::Workspace];
}

/// Canvas tools, grouped as in Moho Pro's Tools panel.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Tool {
    SelectPoints,
    TranslatePoints,
    AddPoint,
    Freehand,
    Eraser,
    PaintBucket,
    Eyedropper,
    #[default]
    TransformLayer,
    SelectBone,
    AddBone,
    ManipulateBones,
    Pan,
    Zoom,
}

impl Tool {
    pub const ALL: [Tool; 13] = [
        Self::SelectPoints,
        Self::TranslatePoints,
        Self::AddPoint,
        Self::Freehand,
        Self::Eraser,
        Self::PaintBucket,
        Self::Eyedropper,
        Self::TransformLayer,
        Self::SelectBone,
        Self::AddBone,
        Self::ManipulateBones,
        Self::Pan,
        Self::Zoom,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::SelectPoints => "Select Points",
            Self::TranslatePoints => "Transform Points",
            Self::AddPoint => "Add Point",
            Self::Freehand => "Freehand",
            Self::Eraser => "Eraser",
            Self::PaintBucket => "Paint Bucket",
            Self::Eyedropper => "Eyedropper",
            Self::TransformLayer => "Transform Layer",
            Self::SelectBone => "Select Bone",
            Self::AddBone => "Add Bone",
            Self::ManipulateBones => "Manipulate Bones",
            Self::Pan => "Pan",
            Self::Zoom => "Zoom",
        }
    }

    pub fn group(self) -> ToolGroup {
        match self {
            Self::SelectPoints | Self::TranslatePoints | Self::AddPoint | Self::Freehand | Self::Eraser => {
                ToolGroup::Draw
            }
            Self::PaintBucket | Self::Eyedropper => ToolGroup::Fill,
            Self::TransformLayer => ToolGroup::Layer,
            Self::SelectBone | Self::AddBone | Self::ManipulateBones => ToolGroup::Bone,
            Self::Pan | Self::Zoom => ToolGroup::Workspace,
        }
    }

    /// Whether the tool works yet. Others are shown so the layout is
    /// complete, and say which milestone brings them.
    pub fn is_available(self) -> bool {
        matches!(self, Self::TransformLayer | Self::Pan | Self::Zoom)
    }

    /// Status-bar help while the tool is active.
    pub fn hint(self) -> &'static str {
        match self {
            Self::TransformLayer => {
                "Drag to move the selected layer. If the layer is animated, a keyframe is set on the current frame."
            }
            Self::Pan => "Drag to pan the canvas. Middle-drag pans with any tool.",
            Self::Zoom => "Click to zoom in, Alt-click to zoom out. Ctrl+scroll zooms with any tool.",
            Self::SelectPoints
            | Self::TranslatePoints
            | Self::AddPoint
            | Self::Freehand
            | Self::Eraser
            | Self::PaintBucket
            | Self::Eyedropper => "Drawing tools arrive in milestone M2.",
            Self::SelectBone | Self::AddBone | Self::ManipulateBones => "Bone tools arrive in milestone M3.",
        }
    }
}
