use core::ffi;

use esp_idf_svc::eventloop::*;

use std::time::Duration;

#[derive(Copy, Clone, Debug)]
pub struct EventLoopMessage<'s> {
    pub duration: Duration,
    pub data: &'s str,
}

impl<'s> EventLoopMessage<'s> {
    pub fn new(duration: Duration, data: &'s str) -> Self {
        Self { duration, data }
    }
}

impl EspTypedEventSource for EventLoopMessage<'_> {
    fn source() -> *const ffi::c_char {
        b"DEMO-SERVICE\0".as_ptr() as *const _
    }
}

impl EspTypedEventSerializer<EventLoopMessage<'_>> for EventLoopMessage<'_> {
    fn serialize<R>(
        event: &EventLoopMessage,
        f: impl for<'a> FnOnce(&'a EspEventPostData) -> R,
    ) -> R {
        f(&unsafe { EspEventPostData::new(Self::source(), Self::event_id(), event) })
    }
}

impl<'s> EspTypedEventDeserializer<EventLoopMessage<'s>> for EventLoopMessage<'s> {
    fn deserialize<R>(
        data: &EspEventFetchData,
        f: &mut impl for<'a> FnMut(&'a EventLoopMessage<'s>) -> R,
    ) -> R {
        f(unsafe { data.as_payload() })
    }
}
