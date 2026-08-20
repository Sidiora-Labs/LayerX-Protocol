/**
 * Event bindings.
 *
 * Events are emitted under the calling program's namespace and the invoking
 * principal. Topic and payload bounds are checked before the host call, so an
 * oversized event never reaches the runtime.
 */

import { MAX_EVENT_DATA_BYTES, MAX_EVENT_TOPIC_BYTES } from "./abi";
import { pointer } from "./bytes";
import { ERR_DATA_TOO_LARGE, ERR_EMPTY_TOPIC, ERR_TOPIC_TOO_LARGE } from "./error";
import { eventEmit } from "./host";

/** Emits one event under this program's namespace. */
export function emitEvent(topic: StaticArray<u8>, data: StaticArray<u8>): i32 {
  if (topic.length == 0) return ERR_EMPTY_TOPIC;
  if (topic.length > MAX_EVENT_TOPIC_BYTES) return ERR_TOPIC_TOO_LARGE;
  if (data.length > MAX_EVENT_DATA_BYTES) return ERR_DATA_TOO_LARGE;
  return eventEmit(pointer(topic), topic.length, pointer(data), data.length);
}
