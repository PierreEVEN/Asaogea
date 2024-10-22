use std::collections::{HashMap, HashSet};
use std::env;
use std::fs::File;
use std::io::BufReader;
use crate::application::gfx::resources::image::Image;
use crate::application::gfx::resources::mesh::{DynamicMesh, IndexBufferType};
use anyhow::{anyhow, Error};
use std::path::{Path, PathBuf};
use base64::Engine;
use base64::prelude::BASE64_STANDARD;
use glam::Vec3;
use gltf::{Document, Gltf};
use gltf::mesh::util::ReadIndices;
use crate::application::gfx::resources::buffer::BufferMemory;

pub struct GltfImporter {
    textures: HashMap<usize, Image>,
    meshes: HashMap<usize, DynamicMesh>,
    buffers: Vec<gltf::buffer::Data>,
    document: Document,
    path: PathBuf,
}

impl GltfImporter {
    pub fn new(path: PathBuf) -> Result<Self, Error> {
        let Gltf { document, mut blob } = Gltf::open("resources/models/sample/Lantern.glb")?;
        let mut buffers = Vec::new();
        for buffer in document.buffers() {
            let mut data = match buffer.source() {
                gltf::buffer::Source::Uri(uri) => GltfDependencyUri::load_from_uri(path.parent().unwrap(), uri),
                gltf::buffer::Source::Bin => blob.take().ok_or(anyhow!("Trying to load bin buffer but it is not valid")),
            }?;
            if data.len() < buffer.length() {
                return Err(anyhow!("Invalid buffer : {} (expected at least {} bytes)", data.len(), buffer.length()));
            }
            while data.len() % 4 != 0 { data.push(0); }
            buffers.push(gltf::buffer::Data(data));
        }

        Ok(Self {
            textures: Default::default(),
            meshes: Default::default(),
            buffers,
            document,
            path,
        })
    }

    pub fn load_texture(&mut self, index: usize) {
        
    }

    pub fn load_mesh(&mut self, index: usize) -> Result<&DynamicMesh, Error> {
        if let Some(mesh) = self.meshes.get(&index) {
            return Ok(mesh);
        }
        let mut dyn_mesh = DynamicMesh::new(size_of::<Vec3>(), ctx.get().device().clone())?;

        for mesh in self.document.meshes() {
            for primitive in mesh.primitives() {
                let mut vertices = vec![];

                let reader = primitive.reader(|data| Some(&self.buffers[data.index()]));
                let positions = reader.read_positions().unwrap();
                vertices.reserve(positions.len());
                for position in positions {
                    vertices.push(position);
                }

                match reader.read_indices() {
                    None => {
                        dyn_mesh.set_vertices(0, &BufferMemory::from_vec(&vertices))?;
                    }
                    Some(indices) => {
                        match indices {
                            ReadIndices::U8(indices) => {
                                dyn_mesh = dyn_mesh.index_type(IndexBufferType::Uint8);
                                let indices: Vec<u8> = indices.collect();
                                dyn_mesh.set_indexed_vertices(0, &BufferMemory::from_vec(&vertices), 0, &BufferMemory::from_vec(&indices))?;
                            }
                            ReadIndices::U16(indices) => {
                                dyn_mesh = dyn_mesh.index_type(IndexBufferType::Uint16);
                                let indices: Vec<u16> = indices.collect();
                                dyn_mesh.set_indexed_vertices(0, &BufferMemory::from_vec(&vertices), 0, &BufferMemory::from_vec(&indices))?;
                            }
                            ReadIndices::U32(indices) => {
                                dyn_mesh = dyn_mesh.index_type(IndexBufferType::Uint32);
                                let indices: Vec<u32> = indices.collect();
                                dyn_mesh.set_indexed_vertices(0, &BufferMemory::from_vec(&vertices), 0, &BufferMemory::from_vec(&indices))?;
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
enum GltfDependencyUri<'a> {
    Base64(Option<&'a str>, &'a str),
    Absolute(&'a str),
    Relative,
}

impl<'a> GltfDependencyUri<'a> {
    fn load_from_uri(base: &Path, uri: &str) -> Result<Vec<u8>, Error> {
        match GltfDependencyUri::parse(uri)? {
            GltfDependencyUri::Base64(_, base64) => BASE64_STANDARD.decode(base64).map_err(|err| anyhow!("Failed to decode base64 : {err}")),
            GltfDependencyUri::Absolute(path) => Self::load_from_file(PathBuf::from(path)),
            GltfDependencyUri::Relative => {
                if let Ok(from_base) = Self::load_from_file(base.join(uri)) {
                    Ok(from_base)
                } else {
                    Self::load_from_file(env::current_dir()?.join(uri))
                }
            }
            _ => Err(anyhow!("Unknown data type")),
        }
    }

    fn parse(uri: &str) -> Result<GltfDependencyUri<'_>, Error> {
        Ok(if uri.contains(':') {
            if let Some(rest) = uri.strip_prefix("data:") {
                let mut it = rest.split(";base64,");
                match (it.next(), it.next()) {
                    (match0_opt, Some(match1)) => GltfDependencyUri::Base64(match0_opt, match1),
                    (Some(match0), _) => GltfDependencyUri::Base64(None, match0),
                    _ => { return Err(anyhow!("Unsupported uri for gltf dependency (invalid base64 string) : {uri}")); }
                }
            } else if let Some(rest) = uri.strip_prefix("file://") {
                GltfDependencyUri::Absolute(rest)
            } else if let Some(rest) = uri.strip_prefix("file:") {
                GltfDependencyUri::Absolute(rest)
            } else {
                return Err(anyhow!("Unsupported uri for gltf dependency : {uri}"));
            }
        } else {
            GltfDependencyUri::Relative
        })
    }

    fn load_from_file(path: PathBuf) -> Result<Vec<u8>, Error> {
        use std::io::Read;
        let file = File::open(&path).map_err(|e| anyhow!("Failed to read gltf dependency '{}' : {e}", path.display()))?;
        let length = file.metadata().map(|x| x.len() + 1).unwrap_or(0);
        let mut data = Vec::with_capacity(length as usize);
        BufReader::new(file).read_to_end(&mut data).map_err(|e| anyhow!("Failed to read gltf dependency '{}' : {e}", path.display()))?;
        Ok(data)
    }
}
