use minifb::Key;
use crate::mmu::MMU;
use gilrs::{Gilrs, Button, Event};

const NINTENDO_VENDOR_ID: u16 = 0x057E;

pub fn get_input(mmu: &mut MMU, window: &mut minifb::Window, gilrs: &mut Gilrs) {
    let start_joyp = mmu.rb(0xFF00);

    let mut button_state = 0x0Fu8;

    let keyboard_state = get_input_keyboard(window, start_joyp);
    let gamepad_state = get_input_gamepad(start_joyp, gilrs);

    let button_state = keyboard_state & gamepad_state;

    let previous = start_joyp & 0x0F;
    let final_joyp = (start_joyp & 0xF0) | button_state;

    mmu.wb(0xFF00, final_joyp);

    if (previous & !button_state & 0x0F) != 0 {
        let interrupt_flag = mmu.rb(0xFF0F) | 0x10;
        mmu.wb(0xFF0F, interrupt_flag);
    }
}
pub fn get_input_keyboard(window: &mut minifb::Window, joyp: u8) -> u8{
    /* d-pad:
        UP: arrow_up
        DOWN: arrow_down
        LEFT: arrow_left
        RIGHT: arrow_right
       Rest:
        A: Z
        B: X
        Start: Enter
        Select: Right Shift
    */
    let mut button_state = 0xFu8;
    if !get_bit(joyp, 5) {
        if window.is_key_down(Key::Enter){
            button_state = clear_bit(button_state, 3);
        }
        if window.is_key_down(Key::RightShift){
            button_state = clear_bit(button_state, 2);
        }
        if window.is_key_down(Key::X){
            button_state = clear_bit(button_state, 1);
        }
        if window.is_key_down(Key::Z){
            button_state = clear_bit(button_state, 0);
        }
    }
    if !get_bit(joyp, 4) {
        if window.is_key_down(Key::Down){
            button_state = clear_bit(button_state, 3);
        }
        if window.is_key_down(Key::Up){
            button_state = clear_bit(button_state, 2);
        }
        if window.is_key_down(Key::Left){
            button_state = clear_bit(button_state, 1);
        }
        if window.is_key_down(Key::Right){
            button_state = clear_bit(button_state, 0);
        }
    }
    button_state
}

pub fn get_input_gamepad(joyp: u8, gilrs: &mut Gilrs) -> u8{
    let mut button_state = 0xFu8;
    for (_id, pad) in gilrs.gamepads() {
        if !get_bit(joyp, 5) {
            if pad.is_pressed(Button::Select){
                button_state = clear_bit(button_state, 3);
            }
            if pad.is_pressed(Button::Start){
                button_state = clear_bit(button_state, 2);
            }
            if pad.is_pressed(Button::East){
                button_state = clear_bit(button_state, 1);
            }
            if pad.is_pressed(Button::South){
                button_state = clear_bit(button_state, 0);
            }
        }
        if !get_bit(joyp, 4){
            if pad.is_pressed(Button::DPadDown){
                button_state = clear_bit(button_state, 3);
            }
            if pad.is_pressed(Button::DPadUp){
                button_state = clear_bit(button_state, 2);
            }
            if pad.is_pressed(Button::DPadLeft){
                button_state = clear_bit(button_state, 1);
            }
            if pad.is_pressed(Button::DPadRight){
                button_state = clear_bit(button_state, 0);
            }
        }
    }
    button_state
}

pub fn get_bit(data: u8, bit: u8) -> bool {
    if bit > 7 {
        panic!("Can't read bit: {}", bit);
    }
    (data & (1 << bit)) != 0
}

pub fn clear_bit(data: u8, bit: u8) -> u8
{
    if bit > 7 {
        panic!("Can't clear bit: {}", bit);
    }
    data & !(1 << bit)
}

pub fn set_bit(data: u8, bit: u8) -> u8
{
    if bit > 7 {
        panic!("Can't set bit: {}", bit);
    }
    data | (1 << bit)
}