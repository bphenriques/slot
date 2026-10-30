use slot_ui::cast_screen_shadow;

const W: usize = 288;
const H: usize = 144;

/// A grey bezel 20 pixels deep around a clear window.
fn bezel() -> Vec<u8> {
    (0..W * H)
        .flat_map(|i| {
            let (x, y) = (i % W, i / W);
            match (20..W - 20).contains(&x) && (20..H - 20).contains(&y) {
                true => [0, 0, 0, 0],
                false => [90, 90, 90, 255],
            }
        })
        .collect()
}

fn at(rgba: &[u8], x: usize, y: usize) -> [u8; 4] {
    let o = (y * W + x) * 4;
    [rgba[o], rgba[o + 1], rgba[o + 2], rgba[o + 3]]
}

#[test]
fn a_bezel_shades_the_top_and_right_of_its_window_as_light_from_the_top_right_would() {
    let mut rgba = bezel();
    cast_screen_shadow(&mut rgba, W as u32, H as u32);
    let (top, right) = (at(&rgba, W / 2, 20), at(&rgba, W - 21, H / 2));
    let (left, bottom) = (at(&rgba, 20, H / 2), at(&rgba, W / 2, H - 21));
    assert!(top[3] > 60 && right[3] > 60, "top {top:?}, right {right:?}");
    assert!(
        left[3] < 10 && bottom[3] < 10,
        "left {left:?}, bottom {bottom:?}"
    );
    assert_eq!(
        at(&rgba, W / 2, H / 2),
        [0, 0, 0, 0],
        "the shadow reached the middle"
    );
    assert_eq!(
        at(&rgba, 5, 5),
        [90, 90, 90, 255],
        "the bezel itself changed"
    );
}
