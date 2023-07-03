#[derive(Default)]
pub struct IRContext {
    next_register: usize,
}

impl IRContext {
    pub fn clear_register(&mut self) {
        self.next_register = 0;
    }

    pub fn claim_register(&mut self) -> usize {
        let register = self.next_register;
        self.next_register += 1;
        register
    }
}
