use crate::application::gfx::resources::buffer::BufferMemory;
use crate::application::gfx::resources::image::Image;
use crate::application::gfx::resources::mesh::IndexBufferType;
use anyhow::{anyhow, Error};
use base64::prelude::BASE64_STANDARD;
use base64::Engine;
use gltf::mesh::util::ReadIndices;
use gltf::{Document, Gltf};
use std::collections::HashMap;
use std::env;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use image::DynamicImage;
use image::ImageFormat::{Jpeg, Png};

pub struct GltfPrimitiveData {
    pub index: Option<BufferMemory<'static>>,
    pub vertex: BufferMemory<'static>,
    index_type: IndexBufferType,
}

pub struct GltfImporter {
    textures: HashMap<usize, DynamicImage>,
    meshes: Vec<Vec<GltfPrimitiveData>>,
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

    pub fn load_texture(&mut self, image_index: usize) -> Result<&DynamicImage, Error> {
        let guess_format = |encoded_image: &[u8]| image::guess_format(encoded_image).map_err(|e| anyhow!("Failed to decode image format : {e}"));
        let result_image;
        let document_image = self.document.images().nth(image_index).unwrap();
        match document_image.source() {
            gltf::image::Source::Uri { uri, mime_type } if base.is_some() => {
                match GltfDependencyUri::parse(uri)? {
                    GltfDependencyUri::Base64(Some(annoying_case), base64) => {
                        let encoded_image = BASE64_STANDARD.decode(&base64)?;
                        let encoded_format = match annoying_case {
                            "image/png" => Png,
                            "image/jpeg" => Jpeg,
                            _ => guess_format(&encoded_image)?,
                        };
                        let decoded_image = image::load_from_memory_with_format(
                            &encoded_image,
                            encoded_format,
                        )?;
                        return Ok(decoded_image);
                    }
                    _ => {}
                }
                let encoded_image = GltfDependencyUri::load_from_uri(base, uri)?;
                let encoded_format = match mime_type {
                    Some("image/png") => Png,
                    Some("image/jpeg") => Jpeg,
                    Some(_) => guess_format(&encoded_image)?,
                    None => match uri.rsplit('.').next() {
                        Some("png") => Png,
                        Some("jpg") | Some("jpeg") => Jpeg,
                        _ => match guess_format(&encoded_image) {
                            Some(format) => format,
                            None => return Err(anyhow!("unknown format")),
                        },
                    },
                };
                let decoded_image = image::load_from_memory_with_format(&encoded_image, encoded_format)?;
                result_image = decoded_image;
            }
            gltf::image::Source::View { view, mime_type } => {
                let parent_buffer_data = &buffer_data[view.buffer().index()].0;
                let begin = view.offset();
                let end = begin + view.length();
                let encoded_image = &parent_buffer_data[begin..end];
                let encoded_format = match mime_type {
                    "image/png" => Png,
                    "image/jpeg" => Jpeg,
                    _ => match guess_format(encoded_image) {
                        Some(format) => format,
                        None => return Err(anyhow!("unknown format")),
                    },
                };
                let decoded_image =
                    image::load_from_memory_with_format(encoded_image, encoded_format)?;
                result_image = decoded_image;
            }
            _ => return Err(anyhow!("unknown format")),
        }
        Ok(result_image)
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
