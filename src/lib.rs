use std::collections::HashMap;
use std::sync::Arc;

use wgpu::ShaderStages;

pub struct Compute {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue
}

impl Compute {
    pub async fn init() -> Arc<Self> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor { 
            backends: wgpu::Backends::PRIMARY, 
            flags: wgpu::InstanceFlags::empty(), 
            dx12_shader_compiler: wgpu::Dx12Compiler::Fxc, 
            gles_minor_version: wgpu::Gles3MinorVersion::Automatic
        });
    
        let adapter = instance.request_adapter(&wgpu::RequestAdapterOptions::default()).await.unwrap();

        let mut limits = wgpu::Limits::default();
        limits.max_push_constant_size = 4;
        limits.max_storage_buffers_per_shader_stage = 10;

        let mut features = wgpu::Features::PUSH_CONSTANTS;

        features |= wgpu::Features::BGRA8UNORM_STORAGE;
        features |= wgpu::Features::TIMESTAMP_QUERY;
        features |= wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES;
        features |= wgpu::Features::CLEAR_TEXTURE;

        let (device, queue) = adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: None,
                required_features: features,
                required_limits: limits,
                
            }, 
            None
        ).await.unwrap();

        Arc::new(Self {
            instance,
            adapter,
            device,
            queue
        })
    }
}

pub enum BindGroupItem<'a> {
    StorageBuffer { label: &'a str, min_binding_size: u64, read_only: bool },
    UniformBuffer { label: &'a str, min_binding_size: u64 },
    Texture { label: &'a str },
    StorageTexture { label: &'a str, access: wgpu::StorageTextureAccess },
    Sampler { label: &'a str }
}

pub struct PipelineItem<'a> {
    pub label: &'a str,
    pub entry_point: &'a str
}

pub struct ComputeProgram<'a> {
    pub compute: Arc<Compute>,
    pub modules: HashMap<&'a str, wgpu::ShaderModule>,
    pub buffers: HashMap<&'a str, wgpu::Buffer>,
    pub textures: HashMap<&'a str, wgpu::Texture>,
    pub texture_views: HashMap<&'a str, wgpu::TextureView>,
    pub samplers: HashMap<&'a str, wgpu::Sampler>,
    pub bind_groups: HashMap<&'a str, wgpu::BindGroup>,
    pub bind_group_layouts: HashMap<&'a str, wgpu::BindGroupLayout>,
    pub compute_pipelines: HashMap<&'a str, wgpu::ComputePipeline>,
    pub render_pipelines: HashMap<&'a str, wgpu::RenderPipeline>,
}

impl<'a> ComputeProgram<'a> {

    pub fn new(compute: Arc<Compute>) -> Self {
        Self {
            modules: HashMap::new(),
            buffers: HashMap::new(),
            textures: HashMap::new(),
            texture_views: HashMap::new(),
            samplers: HashMap::new(),
            bind_groups: HashMap::new(),
            bind_group_layouts: HashMap::new(),
            compute_pipelines: HashMap::new(),
            render_pipelines: HashMap::new(),
            compute
        }
    }

    pub fn add_module<'b: 'a>(&mut self, label: &'b str, shader: wgpu::ShaderModuleDescriptor) {
        let module = self.compute.device.create_shader_module(shader);
        self.modules.insert(label, module);
    }

    pub fn add_buffer<'b: 'a>(& mut self, label: &'b str, usage: wgpu::BufferUsages, size: u64) {
        let buffer = self.compute.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: size.into(),
            usage,
            mapped_at_creation: false
        });

        self.buffers.insert(label, buffer);
    }

    pub fn add_texture<'b: 'a>(&mut self, label: &'b str, usage: wgpu::TextureUsages, format: wgpu::TextureFormat, size: wgpu::Extent3d) {
        let texture = self.compute.device.create_texture(&wgpu::TextureDescriptor {
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

        self.texture_views.insert(label, view);
        self.textures.insert(label, texture);
        
    }

    pub fn add_sampler<'b: 'a>(&mut self, label: &'b str, descriptor: wgpu::SamplerDescriptor) {
        let sampler = self.compute.device.create_sampler(&descriptor);
        self.samplers.insert(label, sampler);
    }
    
    pub fn add_bind_group<'b: 'a>(&mut self, label: &'b str, items: &[BindGroupItem]) {
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
                        resource: self.buffers[label].as_entire_binding()
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
                        resource: self.buffers[label].as_entire_binding()
                    });
                },
                BindGroupItem::Texture { label } => {
                    let sample_type = self.textures[label].format().sample_type(None, None).unwrap();

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
                        resource: wgpu::BindingResource::TextureView(&self.texture_views[label])
                    });
                },
                BindGroupItem::StorageTexture { label, access } => {
                    let format = self.textures[label].format();
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
                        resource: wgpu::BindingResource::TextureView(&self.texture_views[label])
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
                        resource: wgpu::BindingResource::Sampler(&self.samplers[label])
                    });
                }
            }
        }

        let bind_group_layout = self.compute.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &bind_group_layout_entries
        });

        let bind_group = self.compute.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &bind_group_layout,
            entries: &bind_group_entries
        });

        self.bind_groups.insert(label, bind_group);
        self.bind_group_layouts.insert(label, bind_group_layout);
    }

    pub fn copy_buffer_to_buffer_full<'b: 'a>(&self, encoder: &mut wgpu::CommandEncoder, buffer_a: &'b str, buffer_b: &'b str) {
        encoder.copy_buffer_to_buffer(
            &self.buffers[buffer_a], 
            0, 
            &self.buffers[buffer_b],
            0, 
            self.buffers[buffer_b].size()
        );
    }

    pub fn add_compute_pipelines<'b: 'a>(
        &mut self,
        module: &'b str,
        bind_groups: &[&'b str],
        entry_points: &[&'b str],
        push_constant_ranges: &[wgpu::PushConstantRange]
    ) {
        let bind_group_layouts: Vec<_> = bind_groups
            .iter()
            .map(|x| &self.bind_group_layouts[x])
            .collect();

        let pipeline_layout = self.compute.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &bind_group_layouts,
            push_constant_ranges
        });                
        
        for entry_point in entry_points {
            let pipeline = self.compute.device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: None,
                layout: Some(&pipeline_layout),
                module: &self.modules[module],
                entry_point: &entry_point
            });

            self.compute_pipelines.insert(&entry_point, pipeline);
        }
    }

    pub fn add_render_pipelines<'b: 'a>(
        &mut self,
        module: &'b str,
        bind_groups: &[&'b str],
        entry_points: &[(&'b str, (&'b str, &'b str))],
        push_constant_ranges: &[wgpu::PushConstantRange],
        targets: &[Option<wgpu::ColorTargetState>],
        vertex_buffer_layouts: &[wgpu::VertexBufferLayout]
    ) {
        let bind_group_layouts: Vec<_> = bind_groups
            .iter()
            .map(|x| &self.bind_group_layouts[x])
            .collect();

        let pipeline_layout = self.compute.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &bind_group_layouts,
            push_constant_ranges
        });

        for (name, (vs_entry_point, fs_entry_point)) in entry_points {

            let render_pipeline = self.compute.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: None,
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &self.modules[module],
                    entry_point: &vs_entry_point,
                    buffers: vertex_buffer_layouts
                },
                fragment: Some(wgpu::FragmentState {
                    module: &self.modules[module],
                    entry_point: &fs_entry_point,
                    targets
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
            });

            self.render_pipelines.insert(name, render_pipeline);
        }

    }
}