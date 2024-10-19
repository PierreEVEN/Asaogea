use anyhow::{anyhow, Error};
use hassle_rs::{Dxc, DxcIncludeHandler};
use std::fs;
use std::path::PathBuf;

pub struct IncludeHandler {}

impl IncludeHandler {
    pub fn new() -> Self {
        Self {}
    }
}

impl DxcIncludeHandler for IncludeHandler {
    fn load_source(&mut self, path: String) -> Option<String> {
        let path = PathBuf::from(path);

        if path.is_absolute() {
            if !path.exists() {
                None
            } else {
                match fs::read_to_string(&path) {
                    Ok(data) => { Some(data) }
                    Err(_) => { None }
                }
            }
        } else {
            let search_paths = vec!["./shaders/", "./"];

            for search_path in search_paths {
                let path = PathBuf::from(search_path).join(&path);
                if path.exists() {
                    return match fs::read_to_string(path) {
                        Ok(data) => { Some(data) }
                        Err(_) => { None }
                    }
                }
            }
            None
        }
    }
}

pub enum ShaderStage {
    VERTEX,
    PIXEL,
    COMPUTE,

}


pub enum ShaderProfile {}


pub trait ShaderDefinition {
    fn file_name(&self) -> String;
    fn code(&self, include_handler: &mut IncludeHandler) -> Result<String, Error>;
    fn entry_point(&self) -> &String;
    fn target_profile(&self) -> &String;
}

pub struct ShaderFileDefinition {
    path: PathBuf,
    entry_point: String,
    target_profile: String,
}

impl ShaderFileDefinition {
    pub fn new(path: PathBuf, target_profile: &str) -> Self {
        Self { path, entry_point: "main".to_string(), target_profile: target_profile.to_string() }
    }

    pub fn set_entry_point(mut self, entry_point: String) -> Self {
        self.entry_point = entry_point;
        self
    }
}

impl ShaderDefinition for ShaderFileDefinition {
    fn file_name(&self) -> String {
        self.path.file_name().unwrap().to_str().unwrap().to_string()
    }

    fn code(&self, include_handler: &mut IncludeHandler) -> Result<String, Error> {
        match include_handler.load_source(self.path.to_str().unwrap().to_string()) {
            None => { Err(anyhow!("Failed to load source {}", self.path.display())) }
            Some(data) => { Ok(data) }
        }
    }

    fn entry_point(&self) -> &String {
        &self.entry_point
    }

    fn target_profile(&self) -> &String {
        &self.target_profile
    }
}

pub struct RawShaderDefinition {
    filename: String,
    data: String,
    entry_point: String,
    target_profile: String,
}

impl RawShaderDefinition {
    pub fn new(filename: &str, target_profile: &str, data: String) -> Self {
        Self { filename: filename.to_string(), data, entry_point: "main".to_string(), target_profile: target_profile.to_string() }
    }

    pub fn set_entry_point(mut self, entry_point: String) -> Self {
        self.entry_point = entry_point;
        self
    }
}

impl ShaderDefinition for RawShaderDefinition {
    fn file_name(&self) -> String {
        self.filename.clone()
    }

    fn code(&self, _: &mut IncludeHandler) -> Result<String, Error> {
        Ok(self.data.clone())
    }

    fn entry_point(&self) -> &String {
        &self.entry_point
    }

    fn target_profile(&self) -> &String {
        &self.target_profile
    }
}

pub struct SpirV {
    raw: Vec<u8>,
}

impl SpirV {
    pub fn raw(self) -> Vec<u8> {
        self.raw
    }
}

pub struct HlslCompiler {
    include_handler: IncludeHandler,
}

impl HlslCompiler {
    pub fn new() -> Result<Self, Error> {
        let include_handler = IncludeHandler::new();

        Ok(Self {
            include_handler,
        })
    }

    pub fn compile(&mut self, shader: &dyn ShaderDefinition) -> Result<SpirV, Error> {

        let dxc = Dxc::new(Some(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("lib")))?;
        let compiler = dxc.create_compiler()?;
        let library = dxc.create_library()?;

        let code = shader.code(&mut self.include_handler)?;
        let blob = library.create_blob_with_encoding_from_str(code.as_str())?;

        let result = compiler.compile(
            &blob,
            shader.file_name().as_str(),
            shader.entry_point().as_str(),
            shader.target_profile().as_str(),
            &["-spirv"],
            Some(&mut IncludeHandler::new()),
            &[],
        );

        match result {
            Err(result) => {
                let error_blob = result.0.get_error_buffer()?;
                Err(anyhow!("Shader compilation failed : {}", library.get_blob_as_string(&error_blob.into())?))
            }
            Ok(result) => {
                let result_blob = result.get_result()?;
                Ok(SpirV { raw: result_blob.to_vec() })
            }
        }
    }
}