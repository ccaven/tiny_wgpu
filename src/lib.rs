use std::{collections::HashMap, sync::Arc};
use wgpu::{BufferUsages, ShaderStages};

pub struct Compute {
    pub instance: Arc<wgpu::Instance>,
    pub adapter: Arc<wgpu::Adapter>,
    pub device: Arc<wgpu::Device>,
    pub queue: Arc<wgpu::Queue>
}

impl Compute {
    pub async fn new(features: wgpu::Features, limits: wgpu::Limits) -> Self {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor { 
            backends: wgpu::Backends::PRIMARY, 
            flags: wgpu::InstanceFlags::empty(), 
            dx12_shader_compiler: wgpu::Dx12Compiler::Fxc, 
            gles_minor_version: wgpu::Gles3MinorVersion::Automatic
        });
    
        let adapter = instance.request_adapter(&wgpu::RequestAdapterOptions::default()).await.unwrap();

        let (device, queue) = adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: None,
                required_features: features,
                required_limits: limits,
                
            }, 
            None
        ).await.unwrap();

        Self {
            instance: Arc::new(instance),
            adapter: Arc::new(adapter),
            device: Arc::new(device),
            queue: Arc::new(queue)
        }
    }
}

pub enum BindGroupItem {
    StorageBuffer { label: &'static str, min_binding_size: u64, read_only: bool },
    UniformBuffer { label: &'static str, min_binding_size: u64 },
    Texture { label: &'static str },
    TextureView { label: &'static str, sample_type: wgpu::TextureSampleType },
    StorageTexture { label: &'static str, access: wgpu::StorageTextureAccess },
    Sampler { label: &'static str }
}

pub struct ComputeKernel {
    pub label: &'static str,
    pub entry_point: &'static str
}

pub struct RenderKernel {
    pub label: &'static str,
    pub vertex: &'static str,
    pub fragment: &'static str
}

pub struct Storage {
    pub modules: HashMap<&'static str, wgpu::ShaderModule>,
    pub buffers: HashMap<&'static str, wgpu::Buffer>,
    pub textures: HashMap<&'static str, wgpu::Texture>,
    pub texture_views: HashMap<&'static str, wgpu::TextureView>,
    pub samplers: HashMap<&'static str, wgpu::Sampler>,
    pub bind_groups: HashMap<&'static str, wgpu::BindGroup>,
    pub bind_group_layouts: HashMap<&'static str, wgpu::BindGroupLayout>,
    pub compute_pipelines: HashMap<&'static str, wgpu::ComputePipeline>,
    pub render_pipelines: HashMap<&'static str, wgpu::RenderPipeline>,
    
    pub staging_buffers: HashMap<&'static str, wgpu::Buffer>,
    pub staging_senders: HashMap<&'static str, flume::Sender<Result<(), wgpu::BufferAsyncError>>>,
    pub staging_receivers: HashMap<&'static str, flume::Receiver<Result<(), wgpu::BufferAsyncError>>>
}

impl Default for Storage {
    fn default() -> Self {
        Self { 
            modules: Default::default(), 
            buffers: Default::default(), 
            textures: Default::default(), 
            texture_views: Default::default(), 
            samplers: Default::default(), 
            bind_groups: Default::default(), 
            bind_group_layouts: Default::default(), 
            compute_pipelines: Default::default(), 
            render_pipelines: Default::default(), 
            staging_buffers: Default::default(), 
            staging_senders: Default::default(),
            staging_receivers: Default::default()
        }
    }
}

pub trait ComputeProgram {
    fn storage(&self) -> &Storage;
    fn storage_mut(&mut self) -> &mut Storage;
    fn compute(&self) -> &Compute;

    fn add_buffer(&mut self, label: &'static str, usage: wgpu::BufferUsages, size: u64) {
        let buffer = self.compute().device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: size.into(),
            usage,
            mapped_at_creation: false
        });

        self.storage_mut().buffers.insert(label, buffer);
    }
    
    fn add_module(&mut self, label: &'static str, shader: wgpu::ShaderModuleDescriptor) {
        let module = self.compute().device.create_shader_module(shader);
        self.storage_mut().modules.insert(label, module);
    }
    
    fn add_staging_buffer(&mut self, label: &'static str) {
        let buffer = self.compute().device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
            size: self.storage().buffers[label].size(),
            mapped_at_creation: false
        });

        self.storage_mut().staging_buffers.insert(label, buffer);

        let (sender, receiver) = flume::bounded(1);

        self.storage_mut().staging_senders.insert(label, sender);
        self.storage_mut().staging_receivers.insert(label, receiver);
    }
    
    fn add_texture(&mut self, label: &'static str, usage: wgpu::TextureUsages, format: wgpu::TextureFormat, size: wgpu::Extent3d) {
        let texture = self.compute().device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size,
            usage,
            format,
            dimension: wgpu::TextureDimension::D2,
            mip_level_count: 1,
            sample_count: 1,
            view_formats: &[]
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        self.storage_mut().texture_views.insert(label, view);
        self.storage_mut().textures.insert(label, texture);
    }
    
    fn add_sampler(&mut self, label: &'static str, descriptor: wgpu::SamplerDescriptor) {
        let sampler = self.compute().device.create_sampler(&descriptor);
        self.storage_mut().samplers.insert(label, sampler);
    }
    
    fn add_bind_group(&mut self, label: &'static str, items: &[BindGroupItem]) {
        let mut bind_group_layout_entries = Vec::new();
        let mut bind_group_entries = Vec::new();

        for (i, bind_group_item) in items.iter().enumerate() {
            match bind_group_item {
                BindGroupItem::StorageBuffer { label, min_binding_size, read_only } => {
                    bind_group_layout_entries.push(wgpu::BindGroupLayoutEntry {
                        binding: i as u32,
                        // Cannot use storage buffers in vertex shader without feature flag
                        visibility: wgpu::ShaderStages::COMPUTE | wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer { 
                            ty: wgpu::BufferBindingType::Storage { read_only: *read_only }, 
                            has_dynamic_offset: false, 
                            min_binding_size: Some(std::num::NonZeroU64::new(*min_binding_size).unwrap())
                        },
                        count: None
                    });

                    bind_group_entries.push(wgpu::BindGroupEntry {
                        binding: i as u32,
                        resource: self.storage().buffers[label].as_entire_binding()
                    });
                },
                BindGroupItem::UniformBuffer { label, min_binding_size } => {
                    bind_group_layout_entries.push(wgpu::BindGroupLayoutEntry {
                        binding: i as u32,
                        visibility: wgpu::ShaderStages::all(),
                        ty: wgpu::BindingType::Buffer { 
                            ty: wgpu::BufferBindingType::Uniform, 
                            has_dynamic_offset: false, 
                            min_binding_size: Some(std::num::NonZeroU64::new(*min_binding_size).unwrap())
                        },
                        count: None
                    });

                    bind_group_entries.push(wgpu::BindGroupEntry {
                        binding: i as u32,
                        resource: self.storage().buffers[label].as_entire_binding()
                    });
                },
                BindGroupItem::Texture { label } => {
                    let sample_type = self.storage().textures[label].format().sample_type(None, None).unwrap();

                    bind_group_layout_entries.push(wgpu::BindGroupLayoutEntry {
                        binding: i as u32,
                        visibility: wgpu::ShaderStages::all(),
                        ty: wgpu::BindingType::Texture { 
                            sample_type,
                            view_dimension: wgpu::TextureViewDimension::D2, 
                            multisampled: false
                        },
                        count: None
                    });

                    bind_group_entries.push(wgpu::BindGroupEntry {
                        binding: i as u32,
                        resource: wgpu::BindingResource::TextureView(&self.storage().texture_views[label])
                    });
                },
                BindGroupItem::TextureView { label, sample_type } => {
                    bind_group_layout_entries.push(wgpu::BindGroupLayoutEntry {
                        binding: i as u32,
                        visibility: wgpu::ShaderStages::all(),
                        ty: wgpu::BindingType::Texture { 
                            sample_type: *sample_type,
                            view_dimension: wgpu::TextureViewDimension::D2, 
                            multisampled: false
                        },
                        count: None
                    });

                    bind_group_entries.push(wgpu::BindGroupEntry {
                        binding: i as u32,
                        resource: wgpu::BindingResource::TextureView(&self.storage().texture_views[label])
                    });
                },
                BindGroupItem::StorageTexture { label, access } => {
                    let format = self.storage().textures[label].format();
                    bind_group_layout_entries.push(wgpu::BindGroupLayoutEntry {
                        binding: i as u32,
                        visibility: wgpu::ShaderStages::COMPUTE | wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::StorageTexture { 
                            access: *access, 
                            format, 
                            view_dimension: wgpu::TextureViewDimension::D2
                        },
                        count: None
                    });

                    bind_group_entries.push(wgpu::BindGroupEntry {
                        binding: i as u32,
                        resource: wgpu::BindingResource::TextureView(&self.storage().texture_views[label])
                    });
                },
                BindGroupItem::Sampler { label } => {
                    bind_group_layout_entries.push(wgpu::BindGroupLayoutEntry {
                        binding: i as u32,
                        ty: wgpu::BindingType::Sampler(
                            wgpu::SamplerBindingType::Filtering
                        ),
                        visibility: ShaderStages::all(),
                        count: None
                    });

                    bind_group_entries.push(wgpu::BindGroupEntry {
                        binding: i as u32,
                        resource: wgpu::BindingResource::Sampler(&self.storage().samplers[label])
                    });
                }
            }
        }

        let bind_group_layout = self.compute().device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &bind_group_layout_entries
        });

        let bind_group = self.compute().device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &bind_group_layout,
            entries: &bind_group_entries
        });

        self.storage_mut().bind_groups.insert(label, bind_group);
        self.storage_mut().bind_group_layouts.insert(label, bind_group_layout);
    }
    
    fn copy_buffer_to_buffer_full(&self, encoder: &mut wgpu::CommandEncoder, buffer_a: &'static str, buffer_b: &'static str) {
        encoder.copy_buffer_to_buffer(
            &self.storage().buffers[buffer_a], 
            0, 
            &self.storage().buffers[buffer_b],
            0, 
            self.storage().buffers[buffer_b].size()
        );
    }
    
    fn copy_buffer_to_staging(&self, encoder: &mut wgpu::CommandEncoder, label: &'static str) {
        encoder.copy_buffer_to_buffer(
            &self.storage().buffers[label], 
            0, 
            &self.storage().staging_buffers[label],
            0, 
            self.storage().buffers[label].size()
        );
    }
    
    fn prepare_staging_buffer(&self, label: &'static str) {
        let slice = self.storage().staging_buffers[label].slice(..);
        let sender = self.storage().staging_senders[label].clone();
        slice.map_async(wgpu::MapMode::Read, move |v| sender.send(v).unwrap());
    }
    
    fn read_staging_buffer(&self, label: &'static str, dst: &mut [u8]) {
        // Wait for the mapping to finish
        self.storage().staging_receivers[label].recv().unwrap().unwrap();

        // Read data
        {
            let data = self.storage().staging_buffers[label].slice(..).get_mapped_range();
            dst.copy_from_slice(&data);
        }

        // Unmap for the GPU to use again
        self.storage().staging_buffers[label].unmap();
    }
    
    fn add_compute_pipelines(
        &mut self,
        module: &'static str,
        bind_groups: &[&'static str],
        kernels: &[ComputeKernel],
        push_constant_ranges: &[wgpu::PushConstantRange],
        compilation_options: Option<wgpu::PipelineCompilationOptions>
    ) {
        let bind_group_layouts: Vec<_> = bind_groups
            .iter()
            .map(|x| &self.storage().bind_group_layouts[x])
            .collect();

        let pipeline_layout = self.compute().device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &bind_group_layouts,
            push_constant_ranges
        });

        let empty_map = HashMap::new();
        let compilation_options = compilation_options.unwrap_or(wgpu::PipelineCompilationOptions {
            zero_initialize_workgroup_memory: true,
            constants: &empty_map
        });  
        
        for kernel in kernels {
            let pipeline = self.compute().device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: None,
                layout: Some(&pipeline_layout),
                module: &self.storage().modules[module],
                entry_point: &kernel.entry_point,
                compilation_options: compilation_options.clone()
            });

            self.storage_mut().compute_pipelines.insert(&kernel.label, pipeline);
        }
    }

    fn add_render_pipelines_2(
        &mut self,
        
    ) {}
    
    fn add_render_pipelines(
        &mut self,
        module: &'static str,
        bind_groups: &[&'static str],
        kernels: &[RenderKernel],
        push_constant_ranges: &[wgpu::PushConstantRange],
        targets: &[Option<wgpu::ColorTargetState>],
        vertex_buffer_layouts: &[wgpu::VertexBufferLayout],
        vertex_compilation_options: Option<wgpu::PipelineCompilationOptions>,
        fragment_compilation_options: Option<wgpu::PipelineCompilationOptions>
    ) {
        let bind_group_layouts: Vec<_> = bind_groups
            .iter()
            .map(|x| &self.storage().bind_group_layouts[x])
            .collect();

        let pipeline_layout = self.compute().device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &bind_group_layouts,
            push_constant_ranges
        });

        let empty_map = HashMap::new();
        let vertex_compilation_options = vertex_compilation_options.unwrap_or(wgpu::PipelineCompilationOptions { constants: &empty_map, zero_initialize_workgroup_memory: true });
        let fragment_compilation_options = fragment_compilation_options.unwrap_or(wgpu::PipelineCompilationOptions { constants: &empty_map, zero_initialize_workgroup_memory: true });

        for kernel in kernels {
            let render_pipeline = self.compute().device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: None,
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &self.storage().modules[module],
                    entry_point: &kernel.vertex,
                    buffers: vertex_buffer_layouts,
                    compilation_options: vertex_compilation_options.clone()
                },
                fragment: Some(wgpu::FragmentState {
                    module: &self.storage().modules[module],
                    entry_point: &kernel.fragment,
                    targets,
                    compilation_options: fragment_compilation_options.clone()
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
            });

            self.storage_mut().render_pipelines.insert(&kernel.label, render_pipeline);
        }
    }
}