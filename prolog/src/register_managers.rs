use crate::compiler_bytecode::WamReg;

#[allow(unused)]
pub struct RegisterManager {
    count: u32,
}

impl RegisterManager {
    pub fn new() -> Self {
        RegisterManager { count: 0 }
    }

    pub fn get_next(&mut self) -> u32 {
        let current = self.count;
        self.count += 1;
        current
    }

    pub fn reset(&mut self) {
        self.count = 0;
    }
}

pub struct ArgRegisterManager {
    inner: RegisterManager,
}

impl ArgRegisterManager {
    pub fn new() -> Self {
        ArgRegisterManager {
            inner: RegisterManager::new(),
        }
    }

    pub fn get_next(&mut self) -> WamReg {
        WamReg::A(self.inner.get_next())
    }

    pub fn reset(&mut self) {
        self.inner.reset();
    }
}

pub struct XRegisterManager {
    inner: RegisterManager,
}

impl XRegisterManager {
    pub fn new() -> Self {
        XRegisterManager {
            inner: RegisterManager::new(),
        }
    }

    pub fn get_next(&mut self) -> WamReg {
        WamReg::X(self.inner.get_next())
    }

    pub fn reset(&mut self) {
        self.inner.reset();
    }
}

