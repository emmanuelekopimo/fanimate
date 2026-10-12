//! Every command the user can run, and the tools.

/// A command. Menus, shortcuts, context menus and the command palette all
/// run the same actions, so behaviour never depends on how it was invoked.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Action {
    // File
    New,
    Open,
    OpenSample(Sample),
    Save,
    SaveAs,
    Quit,
    // Edit
    Undo,
    Redo,
    /// Take the selected layer back to before its own last change.
    UndoLayer,
    /// Go to a state in the History panel; 0 is the oldest kept.
    GoToHistory(usize),
    Cut,
    Copy,
    Paste,
    Duplicate,
    Delete,
    SelectAll,
    SelectNone,
    Rename,
    /// Finish the path being drawn with the Pen tool.
    FinishPath,
    /// Boolean shape operations on the shapes with selected points.
    Union,
    Subtract,
    Intersect,
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
    /// How the keys on the current frame move on to the next key.
    SetInterpolation(fanimate_core::Interpolation),
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
            OpenSample(Sample::Hello) => "Open Sample: Hello Fanimate",
            OpenSample(Sample::Benchmark) => "Open Sample: Benchmark Rig",
            OpenSample(Sample::StressTest) => "Open Sample: Stress Test",
            Save => "Save",
            SaveAs => "Save As…",
            Quit => "Quit",
            Undo => "Undo",
            Redo => "Redo",
            UndoLayer => "Undo on Selected Layer",
            GoToHistory(_) => "Go to This Step",
            Cut => "Cut",
            Copy => "Copy",
            Paste => "Paste",
            Duplicate => "Duplicate",
            Delete => "Delete",
            SelectAll => "Select All Layers",
            SelectNone => "Select None",
            Rename => "Rename Layer",
            FinishPath => "Finish Path",
            Union => "Union Shapes",
            Subtract => "Subtract Front Shape",
            Intersect => "Intersect Shapes",
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
            SetInterpolation(i) => match i {
                fanimate_core::Interpolation::Smooth => "Interpolation: Smooth",
                fanimate_core::Interpolation::Linear => "Interpolation: Linear",
                fanimate_core::Interpolation::Step => "Interpolation: Step",
            },
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
            UndoLayer,
            Cut,
            Copy,
            Paste,
            Duplicate,
            Delete,
            SelectAll,
            SelectNone,
            Rename,
            FinishPath,
            Union,
            Subtract,
            Intersect,
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
        v.extend(fanimate_core::Interpolation::ALL.map(SetInterpolation));
        v.extend(Sample::ALL.iter().map(|s| OpenSample(*s)));
        v.extend(crate::action::Tool::ALL.iter().map(|t| Tool(*t)));
        v
    }

    /// Whether the action closes the open document, so unsaved changes must
    /// be saved or discarded first.
    pub fn closes_document(self) -> bool {
        matches!(self, Self::New | Self::Open | Self::OpenSample(_) | Self::Quit)
    }
}

/// The sample projects in File › Open Sample.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Sample {
    Hello,
    Benchmark,
    StressTest,
}

impl Sample {
    pub const ALL: [Sample; 3] = [Self::Hello, Self::Benchmark, Self::StressTest];

    /// The name in the Open Sample menu.
    pub fn name(self) -> &'static str {
        match self {
            Self::Hello => "Hello Fanimate",
            Self::Benchmark => "Benchmark rig (40 bones)",
            Self::StressTest => "Stress test (400 bones)",
        }
    }

    pub fn document(self) -> fanimate_core::Document {
        use fanimate_core::samples;
        match self {
            Self::Hello => samples::hello(),
            Self::Benchmark => samples::benchmark(40),
            Self::StressTest => samples::benchmark(400),
        }
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
    Pen,
    Freehand,
    Rectangle,
    Ellipse,
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
    pub const ALL: [Tool; 16] = [
        Self::SelectPoints,
        Self::TranslatePoints,
        Self::AddPoint,
        Self::Pen,
        Self::Freehand,
        Self::Rectangle,
        Self::Ellipse,
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
            Self::Pen => "Pen",
            Self::Freehand => "Freehand",
            Self::Rectangle => "Rectangle",
            Self::Ellipse => "Ellipse",
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
            Self::SelectPoints
            | Self::TranslatePoints
            | Self::AddPoint
            | Self::Pen
            | Self::Freehand
            | Self::Rectangle
            | Self::Ellipse
            | Self::Eraser => ToolGroup::Draw,
            Self::PaintBucket | Self::Eyedropper => ToolGroup::Fill,
            Self::TransformLayer => ToolGroup::Layer,
            Self::SelectBone | Self::AddBone | Self::ManipulateBones => ToolGroup::Bone,
            Self::Pan | Self::Zoom => ToolGroup::Workspace,
        }
    }

    /// Whether the tool works yet. Others are shown so the layout is
    /// complete, and say which milestone brings them.
    pub fn is_available(self) -> bool {
        matches!(
            self,
            Self::SelectPoints
                | Self::TranslatePoints
                | Self::AddPoint
                | Self::Pen
                | Self::Freehand
                | Self::Eraser
                | Self::Rectangle
                | Self::Ellipse
                | Self::PaintBucket
                | Self::Eyedropper
                | Self::TransformLayer
                | Self::Pan
                | Self::Zoom
        )
    }

    /// Whether the tool works on points, so Delete and Select All act on
    /// points rather than layers.
    pub fn edits_points(self) -> bool {
        matches!(self, Self::SelectPoints | Self::TranslatePoints | Self::AddPoint)
    }

    /// Status-bar help while the tool is active.
    pub fn hint(self) -> &'static str {
        match self {
            Self::TransformLayer => {
                "Drag to move the selected layer. If the layer is animated, a keyframe is set on the current frame."
            }
            Self::Freehand => {
                "Draw brush strokes on the selected vector layer. A pen's pressure sets the width; set the size, smoothing and taper in the Style panel."
            }
            Self::Rectangle => "Drag to draw a rectangle. Shift makes a square; Alt draws from the centre.",
            Self::Ellipse => "Drag to draw an ellipse. Shift makes a circle; Alt draws from the centre.",
            Self::PaintBucket => {
                "Click a shape to fill it with the fill colour, or a brush stroke to recolour it with the line colour."
            }
            Self::Eyedropper => "Click a shape to use its colours for new artwork.",
            Self::Eraser => {
                "Drag to erase lines and brush strokes on the selected layer. Shapes it covers completely are removed."
            }
            Self::Pan => "Drag to pan the canvas. Middle-drag pans with any tool.",
            Self::Zoom => "Click to zoom in, Alt-click to zoom out. Ctrl+scroll zooms with any tool.",
            Self::SelectPoints => {
                "Click a point to select it, Shift-click to add, or drag a box around points. Drag selected points to move them."
            }
            Self::TranslatePoints => {
                "Drag to move the selected points, or the nearest point if none are selected."
            }
            Self::AddPoint => {
                "Click a line to add a point on it. Drag from the end of an open path to extend it, or from empty space to start a new line. Drop on the other end to close the shape."
            }
            Self::Pen => {
                "Click to add corner points, drag to add smooth ones. Click the first point to close the shape; Enter or Esc finishes an open path."
            }
            Self::SelectBone | Self::AddBone | Self::ManipulateBones => "Bone tools arrive in milestone M3.",
        }
    }
}
