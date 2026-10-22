//! What the kit gives a host to show: rectangles and pictures, in the order drawn, on a canvas.
//! The host uploads the pictures as the kit makes them (`Bitmap`s), knows them by the `TexId`
//! it hands back, and draws the list however it draws: on a GPU, or into pixels for a test.

/// A picture the host has: the kit names it by this, as the host gave it.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct TexId(pub usize);

/// The room drawn in, in pixels.
#[derive(Copy, Clone, PartialEq, Debug)]
pub struct Canvas {
    pub w: f32,
    pub h: f32,
}

impl Canvas {
    /// A 4:3 panel's, 640 by 480, the music screens' own.
    pub const FRAMED: Canvas = Canvas { w: 640.0, h: 480.0 };

    /// Where something `w` wide starts, to sit in the middle across.
    pub fn centred(self, w: f32) -> f32 {
        (self.w - w) / 2.0
    }
}

/// One thing drawn, over what is drawn before it: a rectangle of a colour, or a picture, faded
/// by `alpha`. Colours are straight RGBA, 0 to 1.
#[derive(Copy, Clone, PartialEq, Debug)]
pub enum Draw {
    Rect {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        colour: [f32; 4],
    },
    Tex {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        tex: TexId,
        alpha: f32,
    },
}
