mod types;

use crate::client::{Client, DynEventFormatter};
use crate::clientmem::{ClientMem, ClientMemOffset};
use crate::format::Format;
use crate::ifs::wl_surface::{WlSurface, WlSurfaceId};
use crate::object::{Interface, Object, ObjectId};
use crate::pixman;
use crate::rect::Rect;
use crate::utils::buffd::MsgParser;
use crate::utils::copyhashmap::CopyHashMap;
use std::rc::Rc;
pub use types::*;

const DESTROY: u32 = 0;

const RELEASE: u32 = 0;

id!(WlBufferId);

pub struct WlBuffer {
    id: WlBufferId,
    pub client: Rc<Client>,
    _offset: usize,
    pub rect: Rect,
    _stride: i32,
    _format: &'static Format,
    pub image: Rc<pixman::Image<ClientMemOffset>>,
    pub(super) surfaces: CopyHashMap<WlSurfaceId, Rc<WlSurface>>,
}

impl WlBuffer {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: WlBufferId,
        client: &Rc<Client>,
        offset: usize,
        width: i32,
        height: i32,
        stride: i32,
        format: &'static Format,
        mem: &Rc<ClientMem>,
    ) -> Result<Self, WlBufferError> {
        let bytes = stride as u64 * height as u64;
        let required = bytes + offset as u64;
        if required > mem.len() as u64 {
            return Err(WlBufferError::OutOfBounds);
        }
        let mem = mem.offset(offset);
        let min_row_size = width as u64 * format.bpp as u64;
        if (stride as u64) < min_row_size {
            return Err(WlBufferError::StrideTooSmall);
        }
        let image = pixman::Image::new(
            mem,
            format.pixman,
            width as u32,
            height as u32,
            stride as u32,
        )?;
        Ok(Self {
            id,
            client: client.clone(),
            _offset: offset,
            rect: Rect::new_sized(0, 0, width, height).unwrap(),
            _stride: stride,
            _format: format,
            image: Rc::new(image),
            surfaces: Default::default(),
        })
    }

    fn destroy(&self, parser: MsgParser<'_, '_>) -> Result<(), DestroyError> {
        let _req: Destroy = self.client.parse(self, parser)?;
        {
            let surfaces = self.surfaces.lock();
            for surface in surfaces.values() {
                surface.buffer.set(None);
            }
        }
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn handle_request_(
        &self,
        request: u32,
        parser: MsgParser<'_, '_>,
    ) -> Result<(), WlBufferError> {
        match request {
            DESTROY => self.destroy(parser)?,
            _ => unreachable!(),
        }
        Ok(())
    }

    pub fn release(self: &Rc<Self>) -> DynEventFormatter {
        Box::new(Release { obj: self.clone() })
    }
}

handle_request!(WlBuffer);

impl Object for WlBuffer {
    fn id(&self) -> ObjectId {
        self.id.into()
    }

    fn interface(&self) -> Interface {
        Interface::WlBuffer
    }

    fn num_requests(&self) -> u32 {
        DESTROY + 1
    }

    fn break_loops(&self) {
        self.surfaces.clear();
    }
}
