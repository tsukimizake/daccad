use crate::compiler_bytecode::WamReg;

#[allow(unused)]
pub(crate) struct RegisterManager {
    count: usize,
}

impl RegisterManager {
    pub fn new() -> Self {
        RegisterManager { count: 0 }
    }

    pub fn get_next(&mut self) -> usize {
        let current = self.count;
        self.count += 1;
        current
    }

    pub fn reset(&mut self) {
        self.count = 0;
    }
}

pub(crate) struct ArgRegisterManager {
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

pub(crate) struct XRegisterManager {
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

