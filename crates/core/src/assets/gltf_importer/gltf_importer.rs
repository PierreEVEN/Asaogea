use crate::core::gfx::resources::buffer::BufferMemory;
use crate::core::gfx::resources::mesh::IndexBufferType;
use anyhow::{anyhow, Error};
use base64::prelude::BASE64_STANDARD;
use base64::Engine;
use gltf::mesh::util::ReadIndices;
use gltf::{Document, Gltf};
use image::DynamicImage;
use image::ImageFormat::{Jpeg, Png};
use std::env;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

pub struct GltfPrimitiveData {
    pub index: Option<BufferMemory<'static>>,
    pub vertex: BufferMemory<'static>,
    pub index_type: IndexBufferType,
}

pub struct GltfImporter {
    meshes: Vec<Vec<GltfPrimitiveData>>,
    buffers: Vec<gltf::buffer::Data>,
    document: Document,
    path: PathBuf,
}

impl GltfImporter {
    pub fn new(path: PathBuf) -> Result<Self, Error> {
        let Gltf { document, mut blob } = Gltf::open(&path)
            .map_err(|e| anyhow!("Failed to open gltf base file {} : {e}", path.display()))?;
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
            meshes: Default::default(),
            buffers,
            document,
            path,
        })
    }

    pub fn num_images(&self) -> usize {
        self.document.images().len()
    }
    
    pub fn load_image(&self, image_index: usize) -> Result<DynamicImage, Error> {
        let document_image = self.document.images().nth(image_index).unwrap().source();
        let image = match document_image {
            gltf::image::Source::Uri { uri, mime_type } => {
                self.load_image_from_data(&GltfDependencyUri::load_from_uri(self.path.parent().unwrap(), uri)?, mime_type)?
            }
            gltf::image::Source::View { view, mime_type } => {
                let parent_buffer_data = &self.buffers[view.buffer().index()].0;
                let begin = view.offset();
                let end = begin + view.length();
                let encoded_image = &parent_buffer_data[begin..end];
                self.load_image_from_data(encoded_image, Some(mime_type))?
            }
        };
        Ok(image)
    }

    fn load_image_from_data(&self, data: &[u8], mime_type: Option<&str>) -> Result<DynamicImage, Error> {
        let encoded_format = if let Some(mime) = mime_type {
            match mime {
                "image/png" => Png,
                "image/jpeg" => Jpeg,
                f => image::guess_format(data).map_err(|e| anyhow!("Failed to guess format of unhandled mime {f} : {e}"))?,
            }
        } else {
            image::guess_format(data).map_err(|e| anyhow!("Failed to guess format : {e}"))?
        };
        Ok(image::load_from_memory_with_format(data, encoded_format)?)
    }

    pub fn get_meshes(&mut self) -> Result<&Vec<Vec<GltfPrimitiveData>>, Error> {
        if !self.meshes.is_empty() {
            return Ok(&self.meshes);
        };

        for mesh in self.document.meshes() {
            let mut primitives = vec![];
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
                        primitives.push(GltfPrimitiveData {
                            index: None,
                            vertex: BufferMemory::from_vec(vertices),
                            index_type: IndexBufferType::default(),
                        });
                    }
                    Some(indices) => {
                        match indices {
                            ReadIndices::U8(indices) => {
                                let indices: Vec<u8> = indices.collect();
                                primitives.push(GltfPrimitiveData {
                                    index: Some(BufferMemory::from_vec(indices)),
                                    vertex: BufferMemory::from_vec(vertices),
                                    index_type: IndexBufferType::Uint8,
                                });
                            }
                            ReadIndices::U16(indices) => {
                                let indices: Vec<u16> = indices.collect();
                                primitives.push(GltfPrimitiveData {
                                    index: Some(BufferMemory::from_vec(indices)),
                                    vertex: BufferMemory::from_vec(vertices),
                                    index_type: IndexBufferType::Uint16,
                                });
                            }
                            ReadIndices::U32(indices) => {
                                let indices: Vec<u32> = indices.collect();
                                primitives.push(GltfPrimitiveData {
                                    index: Some(BufferMemory::from_vec(indices)),
                                    vertex: BufferMemory::from_vec(vertices),
                                    index_type: IndexBufferType::Uint32,
                                });
                            }
                        }
                    }
                }
            }
            self.meshes.push(primitives);
        }
        Ok(&self.meshes)
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
