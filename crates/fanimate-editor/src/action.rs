//! Every command the user can run, and the tools.

/// A command. Menus, shortcuts, context menus and the command palette all
/// run the same actions, so behaviour never depends on how it was invoked.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Action {
    // File
    New,
    Open,
    /// Open a Moho or Adobe Animate project as a new Fanimate project.
    OpenOther,
    OpenSample(Sample),
    Save,
    SaveAs,
    /// Import pictures, Photoshop or SVG files, or sound.
    Import,
    /// Render the animation to a video, GIF or PNG sequence.
    ExportVideo,
    /// Show the render queue window.
    RenderQueue,
    /// Look for a newer version of Fanimate now.
    CheckForUpdates,
    /// Install a downloaded update and restart.
    InstallUpdate,
    /// Arrange the panels for a kind of work.
    Workspace(Workspace),
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
    NewFramesLayer,
    NewSwitchLayer,
    GroupLayers,
    Ungroup,
    MoveLayerUp,
    MoveLayerDown,
    ToggleVisibility,
    LayerProperties,
    /// Give the selected layers this label colour.
    SetLabel(fanimate_core::Label),
    /// Bind shapes rigidly to the selected bone.
    BindToBone,
    /// Bind shapes to the bones around them, with automatic weights.
    BindSmoothly,
    /// Stop shapes following bones.
    Unbind,
    /// A new action on the bone layer, opened for editing.
    NewAction,
    /// A new Smart Bone action for the selected bone, opened for editing.
    NewSmartBoneAction,
    /// Edit action n of the bone layer.
    EditAction(usize),
    /// Stop editing the action and go back to the timeline.
    FinishAction,
    /// Copy action n into the timeline at the current frame.
    InsertAction(usize),
    DeleteAction(usize),
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
    /// A blank drawing on the current frame of a frame-by-frame layer.
    NewDrawing,
    /// Show child n of the selected switch layer from the current frame.
    SetSwitch(usize),
    /// Key the selected switch layer's mouths from a sound, with Rhubarb.
    LipSync,
    /// The sub-layer the tools draw on in frame-by-frame layers.
    SetPart(fanimate_core::Part),
    /// How the keys on the current frame move on to the next key.
    SetInterpolation(fanimate_core::Interpolation),
    // View
    /// Show the drawings before and after this frame faintly.
    ToggleOnionSkin,
    /// Fade every layer but the selected ones.
    ToggleLightTable,
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
            OpenOther => "Open Moho or Animate Project…",
            OpenSample(Sample::Hello) => "Open Sample: Hello Fanimate",
            OpenSample(Sample::Benchmark) => "Open Sample: Benchmark Rig",
            OpenSample(Sample::StressTest) => "Open Sample: Stress Test",
            Save => "Save",
            SaveAs => "Save As…",
            Import => "Import…",
            ExportVideo => "Export Video…",
            RenderQueue => "Render Queue",
            CheckForUpdates => "Check for Updates",
            InstallUpdate => "Install Update and Restart",
            Workspace(w) => w.label(),
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
            NewFramesLayer => "New Frame-by-Frame Layer",
            NewSwitchLayer => "New Switch Layer",
            SetSwitch(_) => "Show This Layer",
            LipSync => "Lip Sync from Sound…",
            NewDrawing => "New Blank Drawing",
            SetPart(p) => match p {
                fanimate_core::Part::Rough => "Draw on Rough",
                fanimate_core::Part::Colour => "Draw on Colour",
                fanimate_core::Part::Line => "Draw on Line",
                fanimate_core::Part::Overlay => "Draw on Overlay",
            },
            GroupLayers => "Group Layers",
            Ungroup => "Ungroup",
            MoveLayerUp => "Move Layer Up",
            MoveLayerDown => "Move Layer Down",
            ToggleVisibility => "Show/Hide Layer",
            LayerProperties => "Layer Properties",
            SetLabel(l) => l.name(),
            BindToBone => "Bind to Selected Bone",
            BindSmoothly => "Bind Smoothly",
            Unbind => "Release from Bones",
            NewAction => "New Action",
            NewSmartBoneAction => "New Smart Bone Action",
            EditAction(_) => "Edit Action",
            FinishAction => "Finish Editing Action",
            InsertAction(_) => "Insert Action Here",
            DeleteAction(_) => "Delete Action",
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
            ToggleOnionSkin => "Onion Skin",
            ToggleLightTable => "Light Table",
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
            OpenOther,
            Save,
            SaveAs,
            Import,
            ExportVideo,
            RenderQueue,
            CheckForUpdates,
            InstallUpdate,
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
            NewFramesLayer,
            NewSwitchLayer,
            GroupLayers,
            Ungroup,
            MoveLayerUp,
            MoveLayerDown,
            ToggleVisibility,
            LayerProperties,
            BindToBone,
            BindSmoothly,
            Unbind,
            NewAction,
            NewSmartBoneAction,
            FinishAction,
            PlayPause,
            NextFrame,
            PrevFrame,
            FirstFrame,
            LastFrame,
            NextKey,
            PrevKey,
            AddKey,
            DeleteKey,
            NewDrawing,
            LipSync,
            ToggleOnionSkin,
            ToggleLightTable,
            ZoomIn,
            ZoomOut,
            FitView,
            ActualSize,
        ];
        v.extend(fanimate_core::Interpolation::ALL.map(SetInterpolation));
        v.extend(fanimate_core::Part::ALL.map(SetPart));
        v.extend(Sample::ALL.iter().map(|s| OpenSample(*s)));
        v.extend(self::Workspace::ALL.map(Action::Workspace));
        v.extend(crate::action::Tool::ALL.iter().map(|t| Tool(*t)));
        v
    }

    /// Whether the action closes the open document, so unsaved changes must
    /// be saved or discarded first.
    pub fn closes_document(self) -> bool {
        matches!(
            self,
            Self::New | Self::Open | Self::OpenOther | Self::OpenSample(_) | Self::InstallUpdate | Self::Quit
        )
    }
}

/// Panel arrangements for kinds of work, in Window › Workspace.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Workspace {
    /// Everything: tools, canvas, timeline, properties and layers.
    Animate,
    /// A large canvas with the Style panel at hand and a short timeline.
    Draw,
    /// Properties and Actions forward, for bones and Smart Bones.
    Rig,
    /// The X-sheet beside a tall timeline, for drawn animation.
    FrameByFrame,
    /// The canvas and the timeline only, for watching and checking.
    Review,
}

impl Workspace {
    pub const ALL: [Workspace; 5] = [
        Self::Animate,
        Self::Draw,
        Self::Rig,
        Self::FrameByFrame,
        Self::Review,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Animate => "Workspace: Animate",
            Self::Draw => "Workspace: Draw",
            Self::Rig => "Workspace: Rig",
            Self::FrameByFrame => "Workspace: Frame by Frame",
            Self::Review => "Workspace: Review",
        }
    }

    /// The name in the Window › Workspace menu.
    pub fn name(self) -> &'static str {
        self.label().trim_start_matches("Workspace: ")
    }
}

/// Command palette search: the actions whose labels match `query`, best
/// first. A label that starts with the query comes first, then labels
/// with a word starting with each query word, then ones containing every
/// word, then ones holding the query's letters in order ("nvl" finds New
/// Vector Layer). Within each, recently used actions (`recent`, most recent
/// first) lead. An empty query lists the recent actions, then the rest.
pub fn search(all: &[Action], query: &str, recent: &[Action]) -> Vec<Action> {
    let query = query.trim().to_lowercase();
    let words: Vec<&str> = query.split_whitespace().collect();
    let rank = |a: &Action| -> Option<u8> {
        if words.is_empty() {
            return Some(0);
        }
        let label = a.label().to_lowercase();
        let label_words: Vec<&str> = label
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| !w.is_empty())
            .collect();
        if label.starts_with(&query) {
            Some(0)
        } else if words.iter().all(|q| label_words.iter().any(|w| w.starts_with(q))) {
            Some(1)
        } else if words.iter().all(|q| label.contains(q)) {
            Some(2)
        } else {
            let mut letters = label.chars();
            let compact: String = query.chars().filter(|c| !c.is_whitespace()).collect();
            compact.chars().all(|q| letters.any(|c| c == q)).then_some(3)
        }
    };
    let recency = |a: &Action| recent.iter().position(|r| r == a).unwrap_or(usize::MAX);
    let mut found: Vec<(u8, usize, usize, Action)> = all
        .iter()
        .enumerate()
        .filter_map(|(i, a)| rank(a).map(|r| (r, recency(a), i, *a)))
        .collect();
    found.sort_by_key(|f| (f.0, f.1, f.2));
    found.into_iter().map(|(.., a)| a).collect()
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
    PaintWeights,
    Pan,
    Zoom,
}

impl Tool {
    pub const ALL: [Tool; 17] = [
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
        Self::PaintWeights,
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
            Self::PaintWeights => "Paint Weights",
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
            Self::SelectBone | Self::AddBone | Self::ManipulateBones | Self::PaintWeights => ToolGroup::Bone,
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
                | Self::SelectBone
                | Self::AddBone
                | Self::ManipulateBones
                | Self::PaintWeights
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
            Self::SelectBone => {
                "Click a bone to select it. Drag its tip to change its length and rest angle, or its joint to move it."
            }
            Self::AddBone => {
                "Drag to draw a bone on the selected bone layer. It joins the selected bone, so drag from its tip to build a chain."
            }
            Self::ManipulateBones => {
                "Drag a bone to pose its chain (inverse kinematics; a pinned bone stops it). Alt-drag turns one bone only. Sets keyframes on the current frame."
            }
            Self::PaintWeights => {
                "Select a bone, then brush over points of a bound vector layer to make them follow that bone more. Alt brushes them away from it."
            }
        }
    }
}
