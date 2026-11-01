use minifb::Key;
use crate::mmu::MMU;
use sdl2::{EventPump, GameControllerSubsystem, Sdl};
use sdl2::controller::{Button, GameController};
use sdl2::event::Event;

const NINTENDO_VENDOR_ID: u16 = 0x057E;


pub struct Pads {
    _sdl: Sdl,
    sub: GameControllerSubsystem,
    pump: EventPump,
    pads: Vec<GameController>,
    held: u8,
    latch: u8,
}

// bit layout: 0=A 1=B 2=Select 3=Start 4=Right 5=Left 6=Up 7=Down
fn bit_for(b: Button) -> Option<u8> {
    Some(match b {
        Button::A => 0,
        Button::B => 1,
        Button::Back => 2,
        Button::Start => 3,
        Button::DPadRight => 4,
        Button::DPadLeft => 5,
        Button::DPadUp => 6,
        Button::DPadDown => 7,
        _ => return None,
    })
}

impl Pads {
    pub fn new() -> Self {
        sdl2::hint::set("SDL_JOYSTICK_ALLOW_BACKGROUND_EVENTS", "1");
        let sdl = sdl2::init().unwrap();
        let sub = sdl.game_controller().unwrap();
        let pump = sdl.event_pump().unwrap();
        Pads { _sdl: sdl, sub, pump, pads: Vec::new(), held: 0, latch: 0 }
    }

    pub fn poll(&mut self) {
        for e in self.pump.poll_iter() {
            match e {
                Event::ControllerDeviceAdded { which, .. } => {
                    if let Ok(p) = self.sub.open(which) {
                        println!("controller connected: {}", p.name());
                        self.pads.push(p);
                    }
                }
                Event::ControllerDeviceRemoved { which, .. } => {
                    self.pads.retain(|p| p.instance_id() != which);
                    self.held = 0;
                }
                Event::ControllerButtonDown { button, .. } => {
                    if let Some(b) = bit_for(button) {
                        self.held |= 1 << b;
                        self.latch |= 1 << b;
                    }
                }
                Event::ControllerButtonUp { button, .. } => {
                    if let Some(b) = bit_for(button) {
                        self.held &= !(1 << b);
                    }
                }
                _ => {}
            }
        }
    }

    pub fn end_frame(&mut self) {
        self.latch = self.held;
    }

    fn pressed(&self) -> u8 {
        self.held | self.latch
    }
}



pub fn get_input(mmu: &mut MMU, window: &mut minifb::Window, input: &mut Pads) {
    let start_joyp = mmu.rb(0xFF00);

    let keyboard_state = get_input_keyboard(window, start_joyp);
    let gamepad_state = get_input_gamepad(start_joyp, input);

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

pub fn get_input_gamepad(joyp: u8, input: &mut Pads) -> u8 {
    let pressed = input.pressed();
    let mut state = 0xFu8;
    if !get_bit(joyp, 5) {
        state &= !(pressed & 0x0F);
    }
    if !get_bit(joyp, 4) {
        state &= !(pressed >> 4);
    }
    state
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