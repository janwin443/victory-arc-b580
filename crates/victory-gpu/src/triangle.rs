//! A first visible D3D12 graphics-pipeline smoke test.
//!
//! Draws one magenta triangle over each presented frame. It is intentionally
//! enabled by default during bring-up; set VICTORY_TEST_TRIANGLE=0 to disable.

use std::sync::Mutex;

use windows::Win32::Graphics::Direct3D::D3D_PRIMITIVE_TOPOLOGY;
use windows::Win32::Graphics::Direct3D12::*;
use windows::Win32::Graphics::Dxgi::Common::*;

const VS: &str = r#"
float4 main(uint id : SV_VertexID) : SV_Position {
    float2 p;
    if (id == 0) p = float2(-0.58, -0.48);
    else if (id == 1) p = float2(0.58, -0.48);
    else p = float2(0.0, 0.62);
    return float4(p, 0.0, 1.0);
}
"#;

const PS: &str = r#"
[RootSignature("RootFlags(0)")]
float4 main() : SV_Target0 {
    return float4(1.0, 0.08, 0.72, 1.0);
}
"#;

struct Triangle {
    root: ID3D12RootSignature,
    pipeline: ID3D12PipelineState,
}

// SAFETY: D3D12 device objects are free-threaded and may be used on the
// presentation thread after creation.
unsafe impl Send for Triangle {}
unsafe impl Sync for Triangle {}

static PIPELINES: Mutex<Vec<(i32, Triangle)>> = Mutex::new(Vec::new());

fn enabled() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var_os("VICTORY_TEST_TRIANGLE").is_none_or(|value| value != "0")
    })
}

fn pipeline(
    device: &ID3D12Device,
    format: DXGI_FORMAT,
) -> windows::core::Result<(ID3D12RootSignature, ID3D12PipelineState)> {
    let vs = crate::indirect::compile_hlsl(VS, "main", "vs_6_0")?;
    let ps = crate::indirect::compile_hlsl(PS, "main", "ps_6_0")?;
    // The pixel shader carries an empty root signature (RootFlags(0)).
    // SAFETY: DXIL remains alive through root-signature and PSO creation.
    unsafe {
        let root: ID3D12RootSignature = device.CreateRootSignature(0, &ps)?;
        let mut blend = D3D12_BLEND_DESC::default();
        blend.RenderTarget[0].RenderTargetWriteMask = 0x0f;
        let mut formats = [DXGI_FORMAT(0); 8];
        formats[0] = format;
        let desc = D3D12_GRAPHICS_PIPELINE_STATE_DESC {
            pRootSignature: std::mem::transmute_copy(&root),
            VS: D3D12_SHADER_BYTECODE {
                pShaderBytecode: vs.as_ptr().cast(),
                BytecodeLength: vs.len(),
            },
            PS: D3D12_SHADER_BYTECODE {
                pShaderBytecode: ps.as_ptr().cast(),
                BytecodeLength: ps.len(),
            },
            BlendState: blend,
            SampleMask: u32::MAX,
            RasterizerState: D3D12_RASTERIZER_DESC {
                FillMode: D3D12_FILL_MODE_SOLID,
                CullMode: D3D12_CULL_MODE_NONE,
                DepthClipEnable: true.into(),
                ..Default::default()
            },
            PrimitiveTopologyType: D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE,
            NumRenderTargets: 1,
            RTVFormats: formats,
            SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
            ..Default::default()
        };
        let pso = device.CreateGraphicsPipelineState(&desc)?;
        Ok((root, pso))
    }
}

/// Record the diagnostic triangle into a back buffer already in
/// RENDER_TARGET state. The PSO is cached per swap-chain format.
pub(crate) fn record(
    host: &crate::host::Host,
    list: &ID3D12GraphicsCommandList,
    back: &ID3D12Resource,
    (width, height, format): (u32, u32, DXGI_FORMAT),
) {
    if !enabled() || width == 0 || height == 0 {
        return;
    }

    let mut made = PIPELINES.lock().unwrap_or_else(|error| error.into_inner());
    if !made.iter().any(|(key, _)| *key == format.0) {
        match pipeline(&host.device, format) {
            Ok((root, pso)) => made.push((format.0, Triangle { root, pipeline: pso })),
            Err(error) => {
                tracing::error!(%error, format = format.0, "could not create DXC triangle pipeline");
                return;
            }
        }
    }
    let Some((_, triangle)) = made.iter().find(|(key, _)| *key == format.0) else {
        return;
    };

    let target = [host.rtv(Some(back), None)];
    // SAFETY: the back buffer is in RENDER_TARGET state and the command list
    // is open. The cached pipeline and root signature outlive this recording.
    unsafe {
        list.OMSetRenderTargets(1, Some(target.as_ptr()), false, None);
        list.SetGraphicsRootSignature(&triangle.root);
        list.SetPipelineState(&triangle.pipeline);
        list.RSSetViewports(&[D3D12_VIEWPORT {
            TopLeftX: 0.0,
            TopLeftY: 0.0,
            Width: width as f32,
            Height: height as f32,
            MinDepth: 0.0,
            MaxDepth: 1.0,
        }]);
        list.RSSetScissorRects(&[windows::Win32::Foundation::RECT {
            left: 0,
            top: 0,
            right: width as i32,
            bottom: height as i32,
        }]);
        list.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY(4));
        list.DrawInstanced(3, 1, 0, 0);
    }
}
