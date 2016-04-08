use chunk::Chunk;

#[derive(Default)]
pub struct Builder {
    signal_name: String,
    trampoline_name: String,
    in_trait: bool,
    function_type_string: String,
}

impl Builder {
    pub fn new() -> Builder {
        Default::default()
    }

    pub fn signal_name(&mut self, name: &str) -> &mut Builder {
        self.signal_name = name.into();
        self
    }

    pub fn trampoline_name(&mut self, name: &str) -> &mut Builder {
        self.trampoline_name = name.into();
        self
    }

    pub fn in_trait(&mut self, value: bool) -> &mut Builder {
        self.in_trait = value;
        self
    }

    pub fn function_type_string(&mut self, type_: &str) -> &mut Builder {
        self.function_type_string = type_.into();
        self
    }

    pub fn generate(&self) -> Chunk {
        let mut body = Vec::new();

        body.push(self.let_func());
        body.push(self.connect());

        let unsafe_ = Chunk::Unsafe(body);

        let mut chunks = Vec::new();
        chunks.push(unsafe_);
        Chunk::BlockHalf(chunks)
    }

    fn let_func(&self) -> Chunk {
        let type_ = format!("Box<Box<{}>>", self.function_type_string);
        Chunk::Let{
            name: "f".to_owned(),
            is_mut: false,
            value: Box::new(Chunk::Custom("Box::new(Box::new(f))".to_owned())),
            type_: Some(Box::new(Chunk::Custom(type_))),
        }
    }

    fn connect(&self) -> Chunk {
        Chunk::Connect{
            signal: self.signal_name.clone(),
            trampoline: self.trampoline_name.clone(),
            in_trait: self.in_trait,
        }
    }
}
