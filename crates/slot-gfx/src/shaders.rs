use crate::surface::GfxError;

const VERT_PREAMBLE: &str = "#version 330 core\n#define attribute in\n#define varying out\n";

const FRAG_PREAMBLE: &str = "#version 330 core\n#define varying in\n\
                             #define texture2D texture\nout vec4 FRAG_COLOR;\n";

const VERT_PREAMBLE_ES: &str = "";
const FRAG_PREAMBLE_ES: &str = "#define FRAG_COLOR gl_FragColor\n";

pub fn program(vert: &str, frag: &str) -> Result<gl::types::GLuint, GfxError> {
    let (vp, fp) = match crate::gl::es() {
        true => (VERT_PREAMBLE_ES, FRAG_PREAMBLE_ES),
        false => (VERT_PREAMBLE, FRAG_PREAMBLE),
    };
    crate::gl::program(&format!("{vp}{vert}"), &format!("{fp}{frag}"))
}

pub const RECT_VERT: &str = r#"
attribute vec2 a_pos;
uniform vec4 u_rect;
uniform vec2 u_target;
varying vec2 v_uv;
void main() {
    v_uv = a_pos;
    vec2 p = (u_rect.xy + a_pos * u_rect.zw) / u_target;
    gl_Position = vec4(p.x * 2.0 - 1.0, 1.0 - p.y * 2.0, 0.0, 1.0);
}
"#;

pub const SPRITE_VERT: &str = r#"
attribute vec2 a_pos;
uniform vec4 u_rect;
uniform vec2 u_target;
uniform vec2 u_turn;
uniform vec4 u_src;
varying vec2 v_uv;
void main() {
    v_uv = u_src.xy + a_pos * u_src.zw;
    vec2 mid = u_rect.zw * 0.5;
    vec2 local = a_pos * u_rect.zw - mid;
    vec2 turned = vec2(u_turn.x * local.x - u_turn.y * local.y,
                       u_turn.y * local.x + u_turn.x * local.y);
    vec2 p = (u_rect.xy + a_pos * u_rect.zw + (turned - local)) / u_target;
    gl_Position = vec4(p.x * 2.0 - 1.0, 1.0 - p.y * 2.0, 0.0, 1.0);
}
"#;

pub const GAME_FRAG: &str = r#"
#ifdef GL_FRAGMENT_PRECISION_HIGH
precision highp float;
#else
precision mediump float;
#endif
uniform sampler2D u_game;
uniform sampler2D u_paper;
uniform vec2 u_src;
uniform vec4 u_uv;
uniform float u_bright;
uniform float u_mode;
uniform float u_paper_size;
uniform vec2 u_origin;
uniform vec4 u_rect;
uniform vec2 u_fbo;
uniform vec4 u_pic;
uniform sampler2D u_ramp;
uniform vec3 u_light;
uniform vec3 u_dark;
uniform mat3 u_response;
uniform vec4 u_lcd;
varying vec2 v_uv;
const vec3 offsets = vec3(3.141592654) * vec3(1.0 / 2.0, 1.0 / 2.0 - 2.0 / 3.0, 1.0 / 2.0 - 4.0 / 3.0);
const vec3 luma_coeff = vec3(0.2126, 0.7152, 0.0722);
vec2 grid_band(vec2 x) {
    return floor(x) / 3.0 + min(fract(x), vec2(1.0 / 3.0));
}
float dot_weight(vec2 f, vec2 o, float bloom) {
    return exp(-4.0 * length(f - (o + vec2(0.5))) * bloom);
}
void main() {
    vec2 uv = u_uv.xy + v_uv * u_uv.zw;
    vec2 f = fract(uv * u_src);
    vec3 c = texture2D(u_game, uv).rgb;
    bool inside = all(greaterThanEqual(uv, u_pic.xy)) && all(lessThan(uv, u_pic.zw));
    if (!inside) {
        // The lit panel runs on past the picture to the edge of the screen: a Game Boy's
        // with its grain, a GBC's reflector white.
        if (u_mode > 4.5) {
            float paper = texture2D(u_paper, (floor(gl_FragCoord.xy - u_origin) + 0.5) / u_paper_size).r;
            c = u_lcd.x > 0.5 ? u_light * (1.0 + (paper - 0.673) * 0.6) : pow(u_light, vec3(1.0 / 2.2));
            c += (1.0 - c) * 0.05 * clamp(1.0 - (v_uv.x * 0.5 + v_uv.y), 0.0, 1.0);
        }
    } else if (u_mode > 4.5) {
        // Authentic: an unlit LCD in room light. See authentic.rs.
        vec2 span = u_rect.zw * u_fbo / (u_uv.zw * u_src);
        // The screen pixel along each game pixel's top and left edge is the gap between dots.
        vec2 gap = step(f * span, vec2(1.0));
        if (u_lcd.x > 0.5) {
            // The core draws grey: how much ink each dot holds over the lit panel. A white
            // dot still shades it a little; a black one lets a little through.
            float a = 0.08 + 0.84 * (1.0 - dot(c, vec3(1.0 / 3.0)));
            a *= gap.x + gap.y > 1.5 ? 0.0 : (gap.x + gap.y > 0.5 ? 0.35 : 1.0);
            // The dots' shadow on the reflector, half a pixel down and to the left as dmg-3x
            // casts it, softened by the glass between them. It shows through pale dots.
            vec2 texel = 1.0 / u_src;
            vec2 from = uv + vec2(0.5, -0.5) * texel;
            float shadow = 0.0;
            for (int j = -1; j <= 1; j++) {
                for (int i = -1; i <= 1; i++) {
                    float w = (i == 0 ? 1.0 : 0.25) * (j == 0 ? 1.0 : 0.25);
                    vec2 at = clamp(from + vec2(float(i), float(j)) * texel, u_pic.xy + texel * 0.5, u_pic.zw - texel * 0.5);
                    shadow += (0.08 + 0.84 * (1.0 - dot(texture2D(u_game, at).rgb, vec3(1.0 / 3.0)))) * w;
                }
            }
            // Over a dot's gaps as well as the dot itself: about six tenths of its ink.
            shadow *= 0.6 / 2.25;
            float paper = texture2D(u_paper, (floor(gl_FragCoord.xy - u_origin) + 0.5) / u_paper_size).r;
            vec3 back = u_light * (1.0 + (paper - 0.673) * 0.6);
            back = mix(back, u_dark, shadow * 0.6);
            c = mix(back, u_dark, a);
        } else {
            // Each 5-bit code's measured light, then the panel's colour response, then the
            // reflector's white and the panel's black.
            vec3 code = (floor(c * 31.0 + 0.5) + 0.5) / 32.0;
            vec2 pick = vec2(1.0 - u_lcd.y, u_lcd.y);
            vec3 light = vec3(dot(texture2D(u_ramp, vec2(code.r, 0.5)).rg, pick),
                              dot(texture2D(u_ramp, vec2(code.g, 0.5)).rg, pick),
                              dot(texture2D(u_ramp, vec2(code.b, 0.5)).rg, pick));
            vec3 lin = clamp(u_response * pow(light, vec3(2.2)), 0.0, 1.0);
            lin = mix(u_dark, u_light, lin);
            lin = mix(lin, u_dark, max(gap.x * u_lcd.z, gap.y * u_lcd.w));
            c = pow(lin, vec3(1.0 / 2.2));
        }
        // The glass catches a soft reflection of the room from the top left.
        c += (1.0 - c) * 0.05 * clamp(1.0 - (v_uv.x * 0.5 + v_uv.y), 0.0, 1.0);
    } else if (u_mode > 3.5) {
        c = pow(c, vec3(1.05));
        vec2 d = abs(f - 0.5);
        float x2 = max(d.x, d.y);
        x2 = x2 * x2;
        float x4 = x2 * x2;
        float w = 48.0 * (x4 - (8.0 / 3.0) * x4 * x2);
        w = w * (w + (1.0 - w) * 0.65) * 0.65;
        float luma = dot(c, luma_coeff);
        w = w * luma;
        c = mix(c, vec3(1.0), w);
        vec3 bg = vec3(texture2D(u_paper, (floor(gl_FragCoord.xy - u_origin) + 0.5) / u_paper_size).r) * c;
        c = mix(c, bg, dot(c, luma_coeff));
    } else if (u_mode > 2.5) {
        float bloom = mix(1.05, 0.95, dot(c, vec3(0.30, 0.59, 0.11)));
        float mid = dot_weight(f, vec2(0.0), bloom);
        float sum = 0.0;
        for (int j = -1; j <= 1; j++) {
            for (int i = -1; i <= 1; i++) {
                sum += dot_weight(f, vec2(float(i), float(j)), bloom);
            }
        }
        c = mix(c, mix(1.2 * c * mid, c * sum, 0.85), 0.85);
    } else if (u_mode > 1.5) {
        vec2 hp = 0.5 * u_uv.zw * u_src / (u_rect.zw * u_fbo);
        vec2 edge = (grid_band(f + hp) - grid_band(f - hp)) / (2.0 * hp);
        c *= 1.0 - 0.75 * max(edge.x, edge.y);
    } else if (u_mode > 0.5) {
        vec2 angle = f * 6.283185307;
        float yfactor = (16.0 + sin(angle.y)) / 17.0;
        vec3 xfactors = (4.0 + sin(angle.x + offsets)) / 5.0;
        c *= yfactor * xfactors;
    }
    FRAG_COLOR = vec4(c * u_bright, 1.0);
}
"#;

pub const SPRITE_FRAG: &str = r#"
precision mediump float;
uniform sampler2D u_tex;
uniform vec4 u_colour;
varying vec2 v_uv;
void main() {
    FRAG_COLOR = texture2D(u_tex, v_uv) * u_colour;
}
"#;

pub const BLIT_VERT: &str = r#"
attribute vec2 a_pos;
uniform vec4 u_uv;
varying vec2 v_uv;
void main() {
    v_uv = u_uv.xy + a_pos * u_uv.zw;
    gl_Position = vec4(a_pos * 2.0 - 1.0, 0.0, 1.0);
}
"#;

pub const TURN_VERT: &str = r#"
attribute vec2 a_pos;
uniform float u_ccw;
varying vec2 v_uv;
void main() {
    vec2 cw = vec2(1.0 - a_pos.y, a_pos.x);
    vec2 ccw = vec2(a_pos.y, 1.0 - a_pos.x);
    v_uv = mix(cw, ccw, u_ccw);
    gl_Position = vec4(a_pos * 2.0 - 1.0, 0.0, 1.0);
}
"#;

pub const BLIT_FRAG: &str = r#"
precision mediump float;
uniform sampler2D u_tex;
uniform vec3 u_gain;
varying vec2 v_uv;
void main() {
    FRAG_COLOR = vec4(texture2D(u_tex, v_uv).rgb * u_gain, 1.0);
}
"#;

/// The LCD's slow response: each channel moves from what the screen showed toward the new
/// frame, more slowly while it turns lighter than while it turns darker.
pub const GHOST_FRAG: &str = r#"
precision mediump float;
uniform sampler2D u_tex;
uniform sampler2D u_prev;
uniform vec2 u_keep;
varying vec2 v_uv;
void main() {
    vec3 now = texture2D(u_tex, v_uv).rgb;
    vec3 was = texture2D(u_prev, v_uv).rgb;
    vec3 keep = mix(vec3(u_keep.y), vec3(u_keep.x), step(was, now));
    vec3 c = mix(now, was, keep);
    // Eight bits cannot hold the last steps of the fade; finish them.
    c = mix(c, now, step(abs(now - was), vec3(1.5 / 255.0)));
    FRAG_COLOR = vec4(c, 1.0);
}
"#;
