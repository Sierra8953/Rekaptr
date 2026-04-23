use collections::FxHashMap;
use etagere::BucketedAtlasAllocator;
use parking_lot::Mutex;
use windows::Win32::Graphics::{
    Direct3D11::{
        D3D11_BIND_SHADER_RESOURCE, D3D11_BOX, D3D11_TEXTURE2D_DESC, D3D11_USAGE_DEFAULT,
        ID3D11Device, ID3D11DeviceContext, ID3D11ShaderResourceView, ID3D11Texture2D,
    },
    Dxgi::Common::*,
};

use crate::{
    AtlasKey, AtlasTextureId, AtlasTextureKind, AtlasTile, Bounds, DevicePixels, PlatformAtlas,
    Point, Size, platform::AtlasTextureList,
};

// Time-based eviction runs per texture, not per tile. A texture is considered
// "in use" as long as any of its tiles are sampled during draw — `get_texture_view`
// refreshes the texture's last_used_frame. This correctly handles scene.replay,
// which re-draws sprites from the previous frame without going through
// `get_or_insert_with`: replay still calls `get_texture_view` during draw, which
// keeps the texture alive.
const EVICT_TEXTURE_AFTER_FRAMES: u64 = 600; // ~10 seconds at 60fps
const EVICT_CHECK_INTERVAL: u64 = 120; // check every ~2 seconds

pub(crate) struct DirectXAtlas(Mutex<DirectXAtlasState>);

struct DirectXAtlasState {
    device: ID3D11Device,
    device_context: ID3D11DeviceContext,
    monochrome_textures: AtlasTextureList<DirectXAtlasTexture>,
    polychrome_textures: AtlasTextureList<DirectXAtlasTexture>,
    tiles_by_key: FxHashMap<AtlasKey, AtlasTile>,
    current_frame: u64,
}

struct DirectXAtlasTexture {
    id: AtlasTextureId,
    bytes_per_pixel: u32,
    allocator: BucketedAtlasAllocator,
    texture: ID3D11Texture2D,
    view: [Option<ID3D11ShaderResourceView>; 1],
    live_atlas_keys: u32,
    last_used_frame: u64,
}

impl DirectXAtlas {
    pub(crate) fn new(device: &ID3D11Device, device_context: &ID3D11DeviceContext) -> Self {
        DirectXAtlas(Mutex::new(DirectXAtlasState {
            device: device.clone(),
            device_context: device_context.clone(),
            monochrome_textures: Default::default(),
            polychrome_textures: Default::default(),
            tiles_by_key: Default::default(),
            current_frame: 0,
        }))
    }

    pub(crate) fn get_texture_view(
        &self,
        id: AtlasTextureId,
    ) -> Option<[Option<ID3D11ShaderResourceView>; 1]> {
        let mut lock = self.0.lock();
        let frame = lock.current_frame;
        let textures = match id.kind {
            AtlasTextureKind::Monochrome => &mut lock.monochrome_textures,
            AtlasTextureKind::Polychrome => &mut lock.polychrome_textures,
        };
        let slot = textures.textures.get_mut(id.index as usize)?;
        let tex = slot.as_mut()?;
        tex.last_used_frame = frame;
        Some(tex.view.clone())
    }

    pub(crate) fn handle_device_lost(
        &self,
        device: &ID3D11Device,
        device_context: &ID3D11DeviceContext,
    ) {
        let mut lock = self.0.lock();
        lock.device = device.clone();
        lock.device_context = device_context.clone();
        lock.monochrome_textures = AtlasTextureList::default();
        lock.polychrome_textures = AtlasTextureList::default();
        lock.tiles_by_key.clear();
    }
}

impl PlatformAtlas for DirectXAtlas {
    fn get_or_insert_with<'a>(
        &self,
        key: &AtlasKey,
        build: &mut dyn FnMut() -> anyhow::Result<
            Option<(Size<DevicePixels>, std::borrow::Cow<'a, [u8]>)>,
        >,
    ) -> anyhow::Result<Option<AtlasTile>> {
        let mut lock = self.0.lock();
        if let Some(tile) = lock.tiles_by_key.get(key) {
            return Ok(Some(tile.clone()));
        }

        let Some((size, bytes)) = build()? else {
            return Ok(None);
        };
        let tile = lock
            .allocate(size, key.texture_kind())
            .ok_or_else(|| anyhow::anyhow!("failed to allocate atlas tile"))?;
        let texture = lock.texture(tile.texture_id);
        texture.upload(&lock.device_context, tile.bounds, &bytes);
        lock.tiles_by_key.insert(key.clone(), tile.clone());
        Ok(Some(tile))
    }

    fn update_tile_from_hardware(
        &self,
        tile: &AtlasTile,
        texture_ptr: *mut std::ffi::c_void,
    ) -> anyhow::Result<()> {
        let mut lock = self.0.lock();
        let frame = lock.current_frame;
        let Some(dst_texture) = lock.try_texture_mut(tile.texture_id) else {
            anyhow::bail!(
                "atlas texture {:?} missing in update_tile_from_hardware",
                tile.texture_id
            );
        };
        dst_texture.last_used_frame = frame;

        unsafe {
            use windows::Win32::Graphics::Direct3D11::ID3D11Resource;
            use windows::core::Interface;
            use std::mem::ManuallyDrop;

            let src_tex: ManuallyDrop<ID3D11Texture2D> = std::mem::transmute_copy(&texture_ptr);
            let src_resource: ID3D11Resource = src_tex.cast()?;
            let dst_resource: ID3D11Resource = dst_texture.texture.cast()?;

            lock.device_context.CopySubresourceRegion(
                &dst_resource,
                0,
                tile.bounds.left().0 as u32,
                tile.bounds.top().0 as u32,
                0,
                &src_resource,
                0,
                None,
            );
        }

        Ok(())
    }

    fn remove(&self, key: &AtlasKey) {
        let mut lock = self.0.lock();
        lock.remove_tile(key);
    }

    fn end_frame(&self) {
        let mut lock = self.0.lock();
        lock.current_frame += 1;

        if lock.current_frame % EVICT_CHECK_INTERVAL == 0 {
            lock.evict_stale_textures();
        }
    }
}

impl DirectXAtlasState {
    fn evict_stale_textures(&mut self) {
        let frame = self.current_frame;
        let threshold = frame.saturating_sub(EVICT_TEXTURE_AFTER_FRAMES);

        let freed_mono = Self::evict_stale_in_list(&mut self.monochrome_textures, threshold);
        let freed_poly = Self::evict_stale_in_list(&mut self.polychrome_textures, threshold);

        if freed_mono.is_empty() && freed_poly.is_empty() {
            return;
        }

        // Drop any tiles_by_key entries whose tile pointed at a freed texture slot.
        // A slot may later be reused by push_texture, which would give different
        // tiles the same AtlasTextureId — so we can't keep stale entries around.
        let before = self.tiles_by_key.len();
        self.tiles_by_key.retain(|_, tile| {
            let freed_for_kind = match tile.texture_id.kind {
                AtlasTextureKind::Monochrome => &freed_mono,
                AtlasTextureKind::Polychrome => &freed_poly,
            };
            !freed_for_kind.contains(&(tile.texture_id.index as usize))
        });
        let after = self.tiles_by_key.len();
        log::debug!(
            "Atlas eviction: {} mono + {} poly textures freed, tiles_by_key {} -> {}",
            freed_mono.len(), freed_poly.len(), before, after
        );
    }

    fn evict_stale_in_list(
        list: &mut AtlasTextureList<DirectXAtlasTexture>,
        threshold: u64,
    ) -> Vec<usize> {
        let mut freed = Vec::new();
        for (idx, slot) in list.textures.iter_mut().enumerate() {
            if let Some(tex) = slot.as_ref()
                && tex.last_used_frame < threshold
            {
                slot.take();
                list.free_list.push(idx);
                freed.push(idx);
            }
        }
        freed
    }

    fn remove_tile(&mut self, key: &AtlasKey) {
        let Some(tile) = self.tiles_by_key.remove(key) else {
            return;
        };

        let texture_id = tile.texture_id;
        let tile_id = tile.tile_id;

        let textures = match texture_id.kind {
            AtlasTextureKind::Monochrome => &mut self.monochrome_textures,
            AtlasTextureKind::Polychrome => &mut self.polychrome_textures,
        };

        let Some(texture_slot) = textures.textures.get_mut(texture_id.index as usize) else {
            return;
        };

        if let Some(texture) = texture_slot.as_mut() {
            texture.allocator.deallocate(tile_id.into());
            texture.decrement_ref_count();
            if texture.is_unreferenced() {
                texture_slot.take();
                textures.free_list.push(texture_id.index as usize);
            }
        }
    }

    fn allocate(
        &mut self,
        size: Size<DevicePixels>,
        texture_kind: AtlasTextureKind,
    ) -> Option<AtlasTile> {
        {
            let textures = match texture_kind {
                AtlasTextureKind::Monochrome => &mut self.monochrome_textures,
                AtlasTextureKind::Polychrome => &mut self.polychrome_textures,
            };

            if let Some(tile) = textures
                .iter_mut()
                .rev()
                .find_map(|texture| texture.allocate(size))
            {
                return Some(tile);
            }
        }

        let texture = self.push_texture(size, texture_kind)?;
        texture.allocate(size)
    }

    fn push_texture(
        &mut self,
        min_size: Size<DevicePixels>,
        kind: AtlasTextureKind,
    ) -> Option<&mut DirectXAtlasTexture> {
        const DEFAULT_ATLAS_SIZE: Size<DevicePixels> = Size {
            width: DevicePixels(1024),
            height: DevicePixels(1024),
        };
        const MAX_ATLAS_SIZE: Size<DevicePixels> = Size {
            width: DevicePixels(16384),
            height: DevicePixels(16384),
        };
        let size = min_size.min(&MAX_ATLAS_SIZE).max(&DEFAULT_ATLAS_SIZE);
        let current_frame = self.current_frame;
        let pixel_format;
        let bind_flag;
        let bytes_per_pixel;
        match kind {
            AtlasTextureKind::Monochrome => {
                pixel_format = DXGI_FORMAT_R8_UNORM;
                bind_flag = D3D11_BIND_SHADER_RESOURCE;
                bytes_per_pixel = 1;
            }
            AtlasTextureKind::Polychrome => {
                pixel_format = DXGI_FORMAT_B8G8R8A8_UNORM;
                bind_flag = D3D11_BIND_SHADER_RESOURCE;
                bytes_per_pixel = 4;
            }
        }
        let texture_desc = D3D11_TEXTURE2D_DESC {
            Width: size.width.0 as u32,
            Height: size.height.0 as u32,
            MipLevels: 1,
            ArraySize: 1,
            Format: pixel_format,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: bind_flag.0 as u32,
            CPUAccessFlags: 0,
            MiscFlags: 0,
        };
        let mut texture: Option<ID3D11Texture2D> = None;
        unsafe {
            self.device
                .CreateTexture2D(&texture_desc, None, Some(&mut texture))
                .ok()?;
        }
        let texture = texture.unwrap();

        let texture_list = match kind {
            AtlasTextureKind::Monochrome => &mut self.monochrome_textures,
            AtlasTextureKind::Polychrome => &mut self.polychrome_textures,
        };
        let index = texture_list.free_list.pop();
        let view = unsafe {
            let mut view = None;
            self.device
                .CreateShaderResourceView(&texture, None, Some(&mut view))
                .ok()?;
            [view]
        };
        let atlas_texture = DirectXAtlasTexture {
            id: AtlasTextureId {
                index: index.unwrap_or(texture_list.textures.len()) as u32,
                kind,
            },
            bytes_per_pixel,
            allocator: etagere::BucketedAtlasAllocator::new(size.into()),
            texture,
            view,
            live_atlas_keys: 0,
            last_used_frame: current_frame,
        };
        if let Some(ix) = index {
            texture_list.textures[ix] = Some(atlas_texture);
            texture_list.textures.get_mut(ix).unwrap().as_mut()
        } else {
            texture_list.textures.push(Some(atlas_texture));
            texture_list.textures.last_mut().unwrap().as_mut()
        }
    }

    fn texture(&self, id: AtlasTextureId) -> &DirectXAtlasTexture {
        let textures = match id.kind {
            crate::AtlasTextureKind::Monochrome => &self.monochrome_textures,
            crate::AtlasTextureKind::Polychrome => &self.polychrome_textures,
        };
        textures[id.index as usize].as_ref().unwrap()
    }

    fn try_texture_mut(&mut self, id: AtlasTextureId) -> Option<&mut DirectXAtlasTexture> {
        let textures = match id.kind {
            crate::AtlasTextureKind::Monochrome => &mut self.monochrome_textures,
            crate::AtlasTextureKind::Polychrome => &mut self.polychrome_textures,
        };
        textures.textures.get_mut(id.index as usize).and_then(|slot| slot.as_mut())
    }
}

impl DirectXAtlasTexture {
    fn allocate(&mut self, size: Size<DevicePixels>) -> Option<AtlasTile> {
        let allocation = self.allocator.allocate(size.into())?;
        let tile = AtlasTile {
            texture_id: self.id,
            tile_id: allocation.id.into(),
            bounds: Bounds {
                origin: allocation.rectangle.min.into(),
                size,
            },
            padding: 0,
        };
        self.live_atlas_keys += 1;
        Some(tile)
    }

    fn upload(
        &self,
        device_context: &ID3D11DeviceContext,
        bounds: Bounds<DevicePixels>,
        bytes: &[u8],
    ) {
        unsafe {
            device_context.UpdateSubresource(
                &self.texture,
                0,
                Some(&D3D11_BOX {
                    left: bounds.left().0 as u32,
                    top: bounds.top().0 as u32,
                    front: 0,
                    right: bounds.right().0 as u32,
                    bottom: bounds.bottom().0 as u32,
                    back: 1,
                }),
                bytes.as_ptr() as _,
                bounds.size.width.to_bytes(self.bytes_per_pixel as u8),
                0,
            );
        }
    }

    fn decrement_ref_count(&mut self) {
        self.live_atlas_keys = self.live_atlas_keys.saturating_sub(1);
    }

    fn is_unreferenced(&mut self) -> bool {
        self.live_atlas_keys == 0
    }
}

impl From<Size<DevicePixels>> for etagere::Size {
    fn from(size: Size<DevicePixels>) -> Self {
        etagere::Size::new(size.width.into(), size.height.into())
    }
}

impl From<etagere::Point> for Point<DevicePixels> {
    fn from(value: etagere::Point) -> Self {
        Point {
            x: DevicePixels::from(value.x),
            y: DevicePixels::from(value.y),
        }
    }
}
