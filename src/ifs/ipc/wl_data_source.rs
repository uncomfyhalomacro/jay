use crate::client::{Client, ClientError};
use crate::ifs::ipc::wl_data_device::WlDataDevice;
use crate::ifs::ipc::wl_data_device_manager::{DND_ALL, DND_NONE};
use crate::ifs::ipc::wl_data_offer::WlDataOffer;
use crate::ifs::ipc::{
    add_mime_type, break_source_loops, cancel_offers, destroy_source, SharedState, SourceData,
    OFFER_STATE_ACCEPTED, OFFER_STATE_DROPPED,
};
use crate::object::Object;
use crate::utils::bitflags::BitflagsExt;
use crate::utils::buffd::MsgParser;
use crate::utils::buffd::MsgParserError;
use crate::wire::wl_data_source::*;
use crate::wire::WlDataSourceId;
use std::rc::Rc;
use thiserror::Error;
use uapi::OwnedFd;

#[allow(dead_code)]
const INVALID_ACTION_MASK: u32 = 0;
#[allow(dead_code)]
const INVALID_SOURCE: u32 = 1;

pub struct WlDataSource {
    pub id: WlDataSourceId,
    pub data: SourceData<WlDataDevice>,
}

impl WlDataSource {
    pub fn new(id: WlDataSourceId, client: &Rc<Client>) -> Self {
        Self {
            id,
            data: SourceData::new(client),
        }
    }

    pub fn on_leave(&self) {
        if self
            .data
            .shared
            .get()
            .state
            .get()
            .contains(OFFER_STATE_DROPPED)
        {
            return;
        }
        self.data.shared.set(Rc::new(SharedState::default()));
        self.send_target(None);
        self.send_action(DND_NONE);
        cancel_offers::<WlDataDevice>(self);
    }

    pub fn update_selected_action(&self) {
        let shared = self.data.shared.get();
        let server_actions = match self.data.actions.get() {
            Some(n) => n,
            _ => {
                log::error!("Server actions not set");
                return;
            }
        };
        let actions = server_actions & shared.receiver_actions.get();
        let action = if actions.contains(shared.receiver_preferred_action.get()) {
            shared.receiver_preferred_action.get()
        } else if actions != 0 {
            1 << actions.trailing_zeros()
        } else {
            0
        };
        if shared.selected_action.replace(action) != action {
            for (_, offer) in &self.data.offers {
                offer.send_action(action);
                offer.client.flush();
            }
            self.send_action(action);
            self.data.client.flush();
        }
    }

    pub fn for_each_data_offer<C: FnMut(&WlDataOffer)>(&self, mut f: C) {
        for (_, offer) in &self.data.offers {
            f(&offer);
        }
    }

    pub fn can_drop(&self) -> bool {
        let shared = self.data.shared.get();
        shared.selected_action.get() != 0 && shared.state.get().contains(OFFER_STATE_ACCEPTED)
    }

    pub fn on_drop(&self) {
        self.send_dnd_drop_performed();
        let shared = self.data.shared.get();
        shared.state.or_assign(OFFER_STATE_DROPPED);
    }

    pub fn send_cancelled(&self) {
        self.data.client.event(Cancelled { self_id: self.id })
    }

    pub fn send_send(&self, mime_type: &str, fd: Rc<OwnedFd>) {
        self.data.client.event(Send {
            self_id: self.id,
            mime_type,
            fd,
        })
    }

    pub fn send_target(&self, mime_type: Option<&str>) {
        self.data.client.event(Target {
            self_id: self.id,
            mime_type,
        })
    }

    pub fn send_dnd_finished(&self) {
        self.data.client.event(DndFinished { self_id: self.id })
    }

    pub fn send_action(&self, dnd_action: u32) {
        self.data.client.event(Action {
            self_id: self.id,
            dnd_action,
        })
    }

    pub fn send_dnd_drop_performed(&self) {
        self.data
            .client
            .event(DndDropPerformed { self_id: self.id })
    }

    fn offer(&self, parser: MsgParser<'_, '_>) -> Result<(), OfferError> {
        let req: Offer = self.data.client.parse(self, parser)?;
        add_mime_type::<WlDataDevice>(self, req.mime_type);
        Ok(())
    }

    fn destroy(&self, parser: MsgParser<'_, '_>) -> Result<(), DestroyError> {
        let _req: Destroy = self.data.client.parse(self, parser)?;
        destroy_source::<WlDataDevice>(self);
        self.data.client.remove_obj(self)?;
        Ok(())
    }

    fn set_actions(&self, parser: MsgParser<'_, '_>) -> Result<(), SetActionsError> {
        let req: SetActions = self.data.client.parse(self, parser)?;
        if self.data.actions.get().is_some() {
            return Err(SetActionsError::AlreadySet);
        }
        if req.dnd_actions & !DND_ALL != 0 {
            return Err(SetActionsError::InvalidActions);
        }
        self.data.actions.set(Some(req.dnd_actions));
        Ok(())
    }
}

object_base! {
    WlDataSource, WlDataSourceError;

    OFFER => offer,
    DESTROY => destroy,
    SET_ACTIONS => set_actions,
}

impl Object for WlDataSource {
    fn num_requests(&self) -> u32 {
        SET_ACTIONS + 1
    }

    fn break_loops(&self) {
        break_source_loops::<WlDataDevice>(self);
    }
}

dedicated_add_obj!(WlDataSource, WlDataSourceId, wl_data_source);

#[derive(Debug, Error)]
pub enum WlDataSourceError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("Could not process `offer` request")]
    OfferError(#[from] OfferError),
    #[error("Could not process `destroy` request")]
    DestroyError(#[from] DestroyError),
    #[error("Could not process `set_actions` request")]
    SetActionsError(#[from] SetActionsError),
}
efrom!(WlDataSourceError, ClientError);

#[derive(Debug, Error)]
pub enum OfferError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(OfferError, ParseFailed, MsgParserError);
efrom!(OfferError, ClientError);

#[derive(Debug, Error)]
pub enum DestroyError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(DestroyError, ParseFailed, MsgParserError);
efrom!(DestroyError, ClientError);

#[derive(Debug, Error)]
pub enum SetActionsError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("The set of actions is invalid")]
    InvalidActions,
    #[error("The actions have already been set")]
    AlreadySet,
}
efrom!(SetActionsError, ParseFailed, MsgParserError);
efrom!(SetActionsError, ClientError);