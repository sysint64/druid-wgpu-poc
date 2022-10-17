// Copyright 2019 The Druid Authors.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! An example of using wgpu.

// On Windows platform, don't show a console when opening the app.
#![windows_subsystem = "windows"]

use std::num::NonZeroU32;
use std::time::Duration;
use std::time::Instant;

use druid::piet::ImageFormat;
use druid::piet::InterpolationMode;
use druid::widget::prelude::*;
use druid::widget::Align;
use druid::widget::Container;
use druid::widget::Label;
use druid::widget::Split;
use druid::ImageBuf;
use druid::{AppLauncher, LocalizedString, TimerToken, WindowDesc};

use wgpu::util::DeviceExt;

static TIMER_INTERVAL: Duration = Duration::from_millis(10);

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    position: [f32; 3],
    color: [f32; 3],
}

impl Vertex {
    fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x3,
                },
            ],
        }
    }
}

const VERTICES: &[Vertex] = &[
    Vertex {
        position: [0.0, 0.5, 0.0],
        color: [1.0, 0.0, 0.0],
    },
    Vertex {
        position: [-0.5, -0.5, 0.0],
        color: [0.0, 1.0, 0.0],
    },
    Vertex {
        position: [0.5, -0.5, 0.0],
        color: [0.0, 0.0, 1.0],
    },
];

struct WgpuWidget {
    timer_id: TimerToken,
    device: wgpu::Device,
    queue: wgpu::Queue,
    render_pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    num_vertices: u32,
    output_buffer: wgpu::Buffer,
    output_buffer_width: u32,
    output_buffer_height: u32,
}

impl WgpuWidget {
    async fn new() -> Self {
        let num_vertices = VERTICES.len() as u32;
        let instance = wgpu::Instance::new(wgpu::Backends::all());
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await
            .unwrap();

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    features: wgpu::Features::empty(),
                    limits: wgpu::Limits::default(),
                    label: None,
                },
                None, // Trace path
            )
            .await
            .unwrap();

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });

        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &[],
                push_constant_ranges: &[],
            });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[Vertex::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba8UnormSrgb,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
        });

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Vertex Buffer"),
            contents: bytemuck::cast_slice(VERTICES),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let output_buffer = WgpuWidget::create_output_buffer(&device, 256, 256);

        Self {
            timer_id: TimerToken::INVALID,
            device,
            queue,
            render_pipeline,
            vertex_buffer,
            num_vertices,
            output_buffer,
            output_buffer_width: 256,
            output_buffer_height: 256,
        }
    }

    fn create_output_buffer(
        device: &wgpu::Device,
        buffer_width: u32,
        buffer_height: u32,
    ) -> wgpu::Buffer {
        let u32_size = std::mem::size_of::<u32>() as u32;

        let output_buffer_size = (u32_size * buffer_width * buffer_height) as wgpu::BufferAddress;
        let output_buffer_desc = wgpu::BufferDescriptor {
            size: output_buffer_size,
            usage: wgpu::BufferUsages::COPY_DST
            // this tells wpgu that we want to read this buffer from the cpu
            | wgpu::BufferUsages::MAP_READ,
            label: None,
            mapped_at_creation: false,
        };
        device.create_buffer(&output_buffer_desc)
    }
}

impl Widget<u32> for WgpuWidget {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event, data: &mut u32, env: &Env) {
        match event {
            Event::WindowConnected => {
                // Start the timer when the application launches
                self.timer_id = ctx.request_timer(TIMER_INTERVAL);
            }
            // Event::Timer(id) => {
            //     if *id == self.timer_id {
            //         ctx.request_layout();
            //         self.timer_id = ctx.request_timer(TIMER_INTERVAL);
            //     }
            // }
            _ => (),
        }
    }

    fn lifecycle(&mut self, ctx: &mut LifeCycleCtx, event: &LifeCycle, data: &u32, env: &Env) {}

    fn update(&mut self, ctx: &mut UpdateCtx, _old_data: &u32, data: &u32, env: &Env) {}

    fn layout(&mut self, ctx: &mut LayoutCtx, bc: &BoxConstraints, data: &u32, env: &Env) -> Size {
        bc.max()
    }

    fn paint(&mut self, ctx: &mut PaintCtx, data: &u32, env: &Env) {
        let i = Instant::now();

        let texture_width = ctx.size().width.ceil() as u32;
        let texture_height = ctx.size().height.ceil() as u32;

        let mut texture_width_padded = texture_width;
        let mut texture_height_padded = texture_height;

        while texture_width_padded % 256 != 0 {
            texture_width_padded += 1;
        }

        while texture_height_padded % 256 != 0 {
            texture_height_padded += 1;
        }

        if texture_width_padded != self.output_buffer_width
            || texture_height_padded != self.output_buffer_height
        {
            self.output_buffer_width = texture_width_padded;
            self.output_buffer_height = texture_height_padded;
            self.output_buffer = WgpuWidget::create_output_buffer(
                &self.device,
                texture_width_padded,
                texture_height_padded,
            );
        }

        let texture_desc = wgpu::TextureDescriptor {
            size: wgpu::Extent3d {
                width: texture_width,
                height: texture_height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::RENDER_ATTACHMENT,
            label: None,
        };

        let texture = self.device.create_texture(&texture_desc);
        let texture_view = texture.create_view(&Default::default());

        // we need to store this for later
        let u32_size = std::mem::size_of::<u32>() as u32;

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &texture_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.1,
                            g: 0.2,
                            b: 0.3,
                            a: 1.0,
                        }),
                        store: true,
                    },
                })],
                depth_stencil_attachment: None,
            });

            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            render_pass.draw(0..self.num_vertices, 0..1);
        }

        encoder.copy_texture_to_buffer(
            wgpu::ImageCopyTexture {
                aspect: wgpu::TextureAspect::All,
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
            },
            wgpu::ImageCopyBuffer {
                buffer: &self.output_buffer,
                layout: wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: NonZeroU32::new(u32_size * texture_width_padded),
                    rows_per_image: NonZeroU32::new(texture_height_padded),
                },
            },
            texture_desc.size,
        );

        self.queue.submit(std::iter::once(encoder.finish()));

        {
            let buffer_slice = self.output_buffer.slice(..);

            let (tx, rx) = futures_intrusive::channel::shared::oneshot_channel();
            buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
                tx.send(result).unwrap();
            });

            self.device.poll(wgpu::Maintain::Wait);

            pollster::block_on(rx.receive()).unwrap().unwrap();

            let data = buffer_slice.get_mapped_range();

            let image_buff = ImageBuf::from_raw(
                &*data,
                ImageFormat::RgbaPremul,
                texture_width_padded as usize,
                texture_height_padded as usize,
            );

            let image = image_buff.to_image(ctx.render_ctx);
            let image_size_padded =
                Size::new(texture_width_padded as f64, texture_height_padded as f64);
            let image_size = Size::new(texture_width as f64, texture_height as f64);
            ctx.with_save(|ctx| {
                ctx.clip(image_size.to_rect());
                ctx.draw_image(
                    &image,
                    image_size_padded.to_rect(),
                    InterpolationMode::NearestNeighbor,
                );
            });
        };
        self.output_buffer.unmap();

        println!("Time: {:?}", i.elapsed());
    }
}

pub fn main() {
    let wgpu_widget = pollster::block_on(WgpuWidget::new());
    let window = WindowDesc::new(Container::new(
        Split::columns(wgpu_widget, Align::centered(Label::new("Right Split")))
            .split_point(0.7)
            .draggable(true),
    ))
    .with_min_size((200., 200.))
    .title(LocalizedString::new("timer-demo-window-title").with_placeholder("Look at it go!"));

    AppLauncher::with_window(window)
        .log_to_console()
        .launch(0u32)
        .expect("launch failed");
}
