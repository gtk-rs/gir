use crate::chunk::Chunk;

#[derive(Default)]
pub struct Builder {
    signal_name: String,
    trampoline_name: String,
    in_trait: bool,
    is_detailed: bool,
}

impl Builder {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn signal_name(&mut self, name: &str) -> &mut Self {
        self.signal_name = name.into();
        self
    }

    pub fn trampoline_name(&mut self, name: &str) -> &mut Self {
        self.trampoline_name = name.into();
        self
    }

    pub fn in_trait(&mut self, value: bool) -> &mut Self {
        self.in_trait = value;
        self
    }

    // https://github.com/rust-lang/rust-clippy/issues/8480
    pub fn is_detailed(&mut self, value: bool) -> &mut Self {
        self.is_detailed = value;
        self
    }

    pub fn generate(&self) -> Chunk {
        let unsafe_ = Chunk::Unsafe(vec![self.let_func(), self.connect()]);

        Chunk::BlockHalf(vec![unsafe_])
    }

    fn let_func(&self) -> Chunk {
        let type_ = "Box_<F>".to_string();
        Chunk::Let {
            name: "f".to_string(),
            is_mut: false,
            value: Box::new(Chunk::Custom("Box_::new(f)".to_owned())),
            type_: Some(Box::new(Chunk::Custom(type_))),
        }
    }

    fn connect(&self) -> Chunk {
        Chunk::Connect {
            signal: self.signal_name.clone(),
            trampoline: self.trampoline_name.clone(),
            in_trait: self.in_trait,
            is_detailed: self.is_detailed,
        }
    }
}
