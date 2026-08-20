from collections.abc import Callable, Iterator
from dataclasses import dataclass
from typing import Generic, TypeVar

T = TypeVar("T")
class StreamCursor(str):
    def __new__(cls, value: str) -> StreamCursor: ...

@dataclass(frozen=True)
class StreamEvent(Generic[T]):
    event_id: str
    previous_cursor: StreamCursor
    cursor: StreamCursor
    value: T

@dataclass(frozen=True)
class StreamPage(Generic[T]):
    requested_cursor: StreamCursor
    events: tuple[StreamEvent[T], ...]
    next_cursor: StreamCursor

class ResumableStream(Generic[T]):
    def __init__(self, cursor: StreamCursor) -> None: ...
    @property
    def cursor(self) -> StreamCursor: ...
    def accept(self, page: StreamPage[T]) -> tuple[StreamEvent[T], ...]: ...
    def events(self, source: Callable[[StreamCursor], StreamPage[T]]) -> Iterator[StreamEvent[T]]: ...
