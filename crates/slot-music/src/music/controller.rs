//! The controller as the music takes it: buttons going down and up, read into the keys the
//! screens know. Two together can be one: any two of the shoulders and triggers are the pocket
//! combo where it is wanted (Now Playing, and the screen off), however a hand finds them blind,
//! one finger squeezing a side's pair or one on each; L1 or R1 with SELECT held skips. The
//! rest are each their own key. What a key then does, tapped or held, is the screens'.

use crate::browse::Key;

/// A button on the controller, as the music takes them; slot keeps its own (MENU, power, the
/// volume, and the lid where there is one).
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Button {
    Up,
    Down,
    Left,
    Right,
    A,
    B,
    X,
    Y,
    L1,
    R1,
    L2,
    R2,
    Start,
    Select,
}

/// What a button did.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Input {
    Down(Button),
    Up(Button),
}

/// A button read as a key: down, or let go.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub(crate) enum Read {
    Press(Key),
    Release(Key),
}

/// The buttons down, to read the next one with.
#[derive(Default)]
pub(crate) struct Controller {
    down: Vec<Button>,
}

impl Controller {
    /// `input`, read as keys; `pocket`, where the pocket combo is taken.
    pub fn read(&mut self, input: Input, pocket: bool) -> Option<Read> {
        match input {
            Input::Down(button) => {
                let with = |other| self.down.contains(&other);
                let key = match button {
                    b if pocket && back(b) && self.down.iter().any(|&d| d != b && back(d)) => {
                        Key::Pocket
                    }
                    Button::L1 if with(Button::Select) => Key::SelectL1,
                    Button::R1 if with(Button::Select) => Key::SelectR1,
                    b => key(b),
                };
                self.down.push(button);
                Some(Read::Press(key))
            }
            // A button let go that was never seen down was down before the music took the keys.
            Input::Up(button) => {
                let was = self.down.iter().position(|&b| b == button)?;
                self.down.remove(was);
                Some(Read::Release(key(button)))
            }
        }
    }

    /// Forgets the buttons down: the screen went off, or the music screen was left.
    pub fn forget(&mut self) {
        self.down.clear();
    }
}

/// Whether `button` is one of the back's: a shoulder or a trigger, any two of which make the
/// pocket combo.
fn back(button: Button) -> bool {
    matches!(button, Button::L1 | Button::R1 | Button::L2 | Button::R2)
}

fn key(button: Button) -> Key {
    match button {
        Button::Up => Key::Up,
        Button::Down => Key::Down,
        Button::Left => Key::Left,
        Button::Right => Key::Right,
        Button::A => Key::A,
        Button::B => Key::B,
        Button::X => Key::X,
        Button::Y => Key::Y,
        Button::L1 => Key::L1,
        Button::R1 => Key::R1,
        Button::L2 => Key::L2,
        Button::R2 => Key::R2,
        Button::Start => Key::Start,
        Button::Select => Key::Select,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(c: &mut Controller, inputs: &[Input], pocket: bool) -> Vec<Read> {
        inputs.iter().filter_map(|&i| c.read(i, pocket)).collect()
    }

    #[test]
    fn any_two_of_the_shoulders_and_triggers_together_are_the_pocket_combo_where_it_is_taken() {
        let mut c = Controller::default();
        let (down, up) = (Input::Down, Input::Up);
        let both = [
            down(Button::R2),
            down(Button::L2),
            up(Button::L2),
            up(Button::R2),
        ];
        assert_eq!(
            read(&mut c, &both, true),
            [
                Read::Press(Key::R2),
                Read::Press(Key::Pocket),
                Read::Release(Key::L2),
                Read::Release(Key::R2)
            ]
        );
        let shoulders = [down(Button::L1), down(Button::R1)];
        assert_eq!(
            read(&mut c, &shoulders, false)[1],
            Read::Press(Key::R1),
            "not taken here"
        );
        c.forget();
        // One finger squeezing a side's pair, as the device logged it: R1, then R2.
        let side = [down(Button::R1), down(Button::R2)];
        assert_eq!(
            read(&mut c, &side, true)[1],
            Read::Press(Key::Pocket),
            "a side's pair"
        );
        c.forget();
        let other = [down(Button::A), down(Button::R2)];
        assert_eq!(
            read(&mut c, &other, true)[1],
            Read::Press(Key::R2),
            "A is no part of it"
        );
    }

    #[test]
    fn a_shoulder_with_select_held_skips_and_a_button_seen_only_let_go_is_none() {
        let mut c = Controller::default();
        let skip = [Input::Down(Button::Select), Input::Down(Button::R1)];
        assert_eq!(read(&mut c, &skip, true)[1], Read::Press(Key::SelectR1));
        assert_eq!(
            c.read(Input::Up(Button::A), true),
            None,
            "down before the music had it"
        );
    }
}
