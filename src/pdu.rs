use crate::Error;
use js_int::UInt;
use ruma::{
    events::{
        pdu::EventHash, room::member::MemberEventContent, AnyEvent, AnyRoomEvent, AnyStateEvent,
        AnyStrippedStateEvent, AnySyncRoomEvent, AnySyncStateEvent, EventType, StateEvent,
    },
    EventId, Raw, RoomId, ServerKeyId, ServerName, UserId,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{collections::BTreeMap, convert::TryInto, sync::Arc, time::UNIX_EPOCH};

#[derive(Deserialize, Serialize, Debug)]
pub struct PduEvent {
    pub event_id: EventId,
    pub room_id: RoomId,
    pub sender: UserId,
    pub origin_server_ts: UInt,
    #[serde(rename = "type")]
    pub kind: EventType,
    pub content: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state_key: Option<String>,
    pub prev_events: Vec<EventId>,
    pub depth: UInt,
    pub auth_events: Vec<EventId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub redacts: Option<EventId>,
    #[serde(default, skip_serializing_if = "serde_json::Map::is_empty")]
    pub unsigned: serde_json::Map<String, serde_json::Value>,
    pub hashes: EventHash,
    pub signatures: BTreeMap<Box<ServerName>, BTreeMap<ServerKeyId, String>>,
}

impl PduEvent {
    pub fn redact(&mut self, reason: &PduEvent) -> crate::Result<()> {
        self.unsigned.clear();

        let allowed: &[&str] = match self.kind {
            EventType::RoomMember => &["membership"],
            EventType::RoomCreate => &["creator"],
            EventType::RoomJoinRules => &["join_rule"],
            EventType::RoomPowerLevels => &[
                "ban",
                "events",
                "events_default",
                "kick",
                "redact",
                "state_default",
                "users",
                "users_default",
            ],
            EventType::RoomHistoryVisibility => &["history_visibility"],
            _ => &[],
        };

        let old_content = self
            .content
            .as_object_mut()
            .ok_or_else(|| Error::bad_database("PDU in db has invalid content."))?;

        let mut new_content = serde_json::Map::new();

        for key in allowed {
            if let Some(value) = old_content.remove(*key) {
                new_content.insert((*key).to_owned(), value);
            }
        }

        self.unsigned.insert(
            "redacted_because".to_owned(),
            serde_json::to_string(reason)
                .expect("PduEvent::to_string always works")
                .into(),
        );

        self.content = new_content.into();

        Ok(())
    }

    pub fn to_sync_room_event(&self) -> Raw<AnySyncRoomEvent> {
        let mut json = json!({
            "content": self.content,
            "type": self.kind,
            "event_id": self.event_id,
            "sender": self.sender,
            "origin_server_ts": self.origin_server_ts,
            "unsigned": self.unsigned,
        });

        if let Some(state_key) = &self.state_key {
            json["state_key"] = json!(state_key);
        }
        if let Some(redacts) = &self.redacts {
            json["redacts"] = json!(redacts);
        }

        serde_json::from_value(json).expect("Raw::from_value always works")
    }

    /// This only works for events that are also AnyRoomEvents.
    pub fn to_any_event(&self) -> Raw<AnyEvent> {
        let mut json = json!({
            "content": self.content,
            "type": self.kind,
            "event_id": self.event_id,
            "sender": self.sender,
            "origin_server_ts": self.origin_server_ts,
            "unsigned": self.unsigned,
            "room_id": self.room_id,
        });

        if let Some(state_key) = &self.state_key {
            json["state_key"] = json!(state_key);
        }
        if let Some(redacts) = &self.redacts {
            json["redacts"] = json!(redacts);
        }

        serde_json::from_value(json).expect("Raw::from_value always works")
    }

    pub fn to_room_event(&self) -> Raw<AnyRoomEvent> {
        let mut json = json!({
            "content": self.content,
            "type": self.kind,
            "event_id": self.event_id,
            "sender": self.sender,
            "origin_server_ts": self.origin_server_ts,
            "unsigned": self.unsigned,
            "room_id": self.room_id,
        });

        if let Some(state_key) = &self.state_key {
            json["state_key"] = json!(state_key);
        }
        if let Some(redacts) = &self.redacts {
            json["redacts"] = json!(redacts);
        }

        serde_json::from_value(json).expect("Raw::from_value always works")
    }

    pub fn to_state_event(&self) -> Raw<AnyStateEvent> {
        let json = json!({
            "content": self.content,
            "type": self.kind,
            "event_id": self.event_id,
            "sender": self.sender,
            "origin_server_ts": self.origin_server_ts,
            "unsigned": self.unsigned,
            "room_id": self.room_id,
            "state_key": self.state_key,
        });

        serde_json::from_value(json).expect("Raw::from_value always works")
    }

    pub fn to_sync_state_event(&self) -> Raw<AnySyncStateEvent> {
        let json = json!({
            "content": self.content,
            "type": self.kind,
            "event_id": self.event_id,
            "sender": self.sender,
            "origin_server_ts": self.origin_server_ts,
            "unsigned": self.unsigned,
            "state_key": self.state_key,
        });

        serde_json::from_value(json).expect("Raw::from_value always works")
    }

    pub fn to_stripped_state_event(&self) -> Raw<AnyStrippedStateEvent> {
        let json = json!({
            "content": self.content,
            "type": self.kind,
            "sender": self.sender,
            "state_key": self.state_key,
        });

        serde_json::from_value(json).expect("Raw::from_value always works")
    }

    pub fn to_member_event(&self) -> Raw<StateEvent<MemberEventContent>> {
        let json = json!({
            "content": self.content,
            "type": self.kind,
            "event_id": self.event_id,
            "sender": self.sender,
            "origin_server_ts": self.origin_server_ts,
            "redacts": self.redacts,
            "unsigned": self.unsigned,
            "room_id": self.room_id,
            "state_key": self.state_key,
        });

        serde_json::from_value(json).expect("Raw::from_value always works")
    }

    pub fn convert_to_outgoing_federation_event(
        mut pdu_json: serde_json::Value,
    ) -> Raw<ruma::events::pdu::PduStub> {
        if let Some(unsigned) = pdu_json
            .as_object_mut()
            .expect("json is object")
            .get_mut("unsigned")
        {
            unsigned
                .as_object_mut()
                .expect("unsigned is object")
                .remove("transaction_id");
        }

        pdu_json
            .as_object_mut()
            .expect("json is object")
            .remove("event_id");

        serde_json::from_value::<Raw<_>>(pdu_json).expect("Raw::from_value always works")
    }
}

impl From<&state_res::StateEvent> for PduEvent {
    fn from(pdu: &state_res::StateEvent) -> Self {
        Self {
            event_id: pdu.event_id().clone(),
            room_id: pdu.room_id().unwrap().clone(),
            sender: pdu.sender().clone(),
            origin_server_ts: (pdu
                .origin_server_ts()
                .duration_since(UNIX_EPOCH)
                .expect("time is valid")
                .as_millis() as u64)
                .try_into()
                .expect("time is valid"),
            kind: pdu.kind(),
            content: pdu.content().clone(),
            state_key: Some(pdu.state_key()),
            prev_events: pdu.prev_event_ids(),
            depth: *pdu.depth(),
            auth_events: pdu.auth_events(),
            redacts: pdu.redacts().cloned(),
            unsigned: pdu.unsigned().clone().into_iter().collect(),
            hashes: pdu.hashes().clone(),
            signatures: pdu.signatures(),
        }
    }
}

impl PduEvent {
    pub fn convert_for_state_res(&self) -> Arc<state_res::StateEvent> {
        Arc::new(
            serde_json::from_value(json!({
                "event_id": self.event_id,
                "room_id": self.room_id,
                "sender": self.sender,
                "origin_server_ts": self.origin_server_ts,
                "type": self.kind,
                "content": self.content,
                "state_key": self.state_key,
                "prev_events": self.prev_events
                    .iter()
                    // TODO How do we create one of these
                    .map(|id| (id, EventHash { sha256: "hello".into() }))
                    .collect::<Vec<_>>(),
                "depth": self.depth,
                "auth_events": self.auth_events
                    .iter()
                    // TODO How do we create one of these
                    .map(|id| (id, EventHash { sha256: "hello".into() }))
                    .collect::<Vec<_>>(),
                "redacts": self.redacts,
                "unsigned": self.unsigned,
                "hashes": self.hashes,
                "signatures": self.signatures,
            }))
            .expect("all conduit PDUs are state events"),
        )
    }
}

/// Build the start of a PDU in order to add it to the `Database`.
#[derive(Debug, Deserialize)]
pub struct PduBuilder {
    #[serde(rename = "type")]
    pub event_type: EventType,
    pub content: serde_json::Value,
    pub unsigned: Option<serde_json::Map<String, serde_json::Value>>,
    pub state_key: Option<String>,
    pub redacts: Option<EventId>,
}
