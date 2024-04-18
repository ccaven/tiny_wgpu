use std::collections::HashMap;
use std::hash::Hash;
use std::sync::Arc;

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

        let (device, queue) = adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: None,
                required_features: wgpu::Features::all_native_mask(),
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

pub enum BindGroupItem<Label> {
    StorageBuffer { label: Label, min_binding_size: u64, read_only: bool },
    UniformBuffer { label: Label, min_binding_size: u64 },
    Texture { label: Label },
    StorageTexture { label: Label, access: wgpu::StorageTextureAccess }
}

pub struct PipelineItem<'a, Label> {
    pub label: Label,
    pub entry_point: &'a str
}

pub struct ComputeProgram<Label: Hash + Eq> {
    pub module: wgpu::ShaderModule,
    pub buffers: HashMap<Label, wgpu::Buffer>,
    pub textures: HashMap<Label, wgpu::Texture>,
    pub texture_views: HashMap<Label, wgpu::TextureView>,
    pub bind_groups: HashMap<Label, wgpu::BindGroup>,
    pub bind_group_layouts: HashMap<Label, wgpu::BindGroupLayout>,
    pub pipelines: HashMap<Label, wgpu::ComputePipeline>,
    pub compute: Arc<Compute>
}

impl<Label: Hash + Eq + Copy> ComputeProgram<Label> {

    pub fn new<'a>(compute: Arc<Compute>, shader_source: wgpu::ShaderModuleDescriptor) -> Self {
        let module = compute.device.create_shader_module(shader_source);

        Self {
            module,
            buffers: HashMap::new(),
            textures: HashMap::new(),
            texture_views: HashMap::new(),
            bind_groups: HashMap::new(),
            bind_group_layouts: HashMap::new(),
            pipelines: HashMap::new(),
            compute
        }
    }

    pub fn add_buffer(&mut self, label: Label, usage: wgpu::BufferUsages, size: u64) {
        let buffer = self.compute.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: size.into(),
            usage,
            mapped_at_creation: false
        });

        self.buffers.insert(label, buffer);
    }

    pub fn add_texture(&mut self, label: Label, usage: wgpu::TextureUsages, format: wgpu::TextureFormat, size: wgpu::Extent3d) {
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

        if usage.contains(wgpu::TextureUsages::TEXTURE_BINDING) {
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            self.texture_views.insert(label.clone(), view);
        }

        self.textures.insert(label, texture);

        
    }
    
    pub fn add_bind_group(&mut self, label: Label, items: Vec<BindGroupItem<Label>>) {
        let mut bind_group_layout_entries = Vec::new();
        let mut bind_group_entries = Vec::new();

        for (i, bind_group_item) in items.iter().enumerate() {
            match bind_group_item {
                BindGroupItem::StorageBuffer { label, min_binding_size, read_only } => {
                    bind_group_layout_entries.push(wgpu::BindGroupLayoutEntry {
                        binding: i as u32,
                        visibility: wgpu::ShaderStages::COMPUTE,
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
                        visibility: wgpu::ShaderStages::COMPUTE,
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
                        visibility: wgpu::ShaderStages::COMPUTE,
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
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::StorageTexture { 
                            access: *access, 
                            format, 
                            view_dimension: wgpu::TextureViewDimension::D2
                        },
                        count: None
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

    pub fn add_pipelines<'a>(
        &mut self, 
        bind_groups: Vec<Label>, 
        items: Vec<PipelineItem<'a, Label>>,
        push_constant_ranges: &'a [wgpu::PushConstantRange]
    ) {
        // Compute pipeline layout
        let bind_group_layouts: Vec<_> = bind_groups
            .iter()
            .map(|x| &self.bind_group_layouts[x])
            .collect();

        let pipeline_layout = self.compute.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &bind_group_layouts,
            push_constant_ranges
        });

        for item in items.iter() {
            let pipeline = self.compute.device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: None,
                layout: Some(&pipeline_layout),
                module: &self.module,
                entry_point: &item.entry_point
            });

            self.pipelines.insert(item.label, pipeline);
        }
    }
}