#!/usr/bin/env python3
import asyncio
import json
from collections import defaultdict

import websockets


EVENTS_BY_ID: dict[str, dict] = {}
SUBSCRIPTIONS: dict[websockets.ServerConnection, dict[str, list[dict]]] = defaultdict(dict)


def log(message: str) -> None:
    print(message, flush=True)


def matches_filter(event: dict, flt: dict) -> bool:
    authors = flt.get("authors")
    if authors and event.get("pubkey") not in authors:
        return False

    kinds = flt.get("kinds")
    if kinds and event.get("kind") not in kinds:
        return False

    return True


def matches_any_filter(event: dict, filters: list[dict]) -> bool:
    if not filters:
        return True
    return any(matches_filter(event, flt) for flt in filters)


async def send_event_to_matching_subscriptions(event: dict) -> None:
    payload = json.dumps(event, separators=(",", ":"))
    stale_connections = []

    for websocket, subscriptions in list(SUBSCRIPTIONS.items()):
        try:
            for subscription_id, filters in subscriptions.items():
                if matches_any_filter(event, filters):
                    log(
                        f"broadcast event kind={event.get('kind')} id={event.get('id')} "
                        f"to sub={subscription_id}"
                    )
                    await websocket.send(f'["EVENT","{subscription_id}",{payload}]')
        except Exception:
            stale_connections.append(websocket)

    for websocket in stale_connections:
        SUBSCRIPTIONS.pop(websocket, None)


async def handle_message(websocket: websockets.ServerConnection, raw_message: str) -> None:
    message = json.loads(raw_message)
    if not isinstance(message, list) or not message:
        return

    kind = message[0]

    if kind == "REQ" and len(message) >= 2:
        subscription_id = message[1]
        filters = [flt for flt in message[2:] if isinstance(flt, dict)]
        SUBSCRIPTIONS[websocket][subscription_id] = filters
        log(f"REQ sub={subscription_id} filters={json.dumps(filters, separators=(',', ':'))}")

        for event in EVENTS_BY_ID.values():
            if matches_any_filter(event, filters):
                payload = json.dumps(event, separators=(",", ":"))
                await websocket.send(f'["EVENT","{subscription_id}",{payload}]')
        await websocket.send(f'["EOSE","{subscription_id}"]')
        return

    if kind == "CLOSE" and len(message) >= 2:
        subscription_id = message[1]
        SUBSCRIPTIONS[websocket].pop(subscription_id, None)
        log(f"CLOSE sub={subscription_id}")
        return

    if kind == "EVENT" and len(message) >= 2 and isinstance(message[1], dict):
        event = message[1]
        event_id = event.get("id")
        if isinstance(event_id, str):
            log(
                f"EVENT kind={event.get('kind')} id={event_id} "
                f"author={event.get('pubkey')}"
            )
            EVENTS_BY_ID[event_id] = event
            await websocket.send(f'["OK","{event_id}",true,""]')
            await send_event_to_matching_subscriptions(event)
        return


async def handler(websocket: websockets.ServerConnection) -> None:
    log("client connected")
    try:
        async for raw_message in websocket:
            await handle_message(websocket, raw_message)
    except websockets.ConnectionClosed:
        log("client disconnected")
    finally:
        SUBSCRIPTIONS.pop(websocket, None)


async def main() -> None:
    async with websockets.serve(handler, "0.0.0.0", 4848, ping_interval=20, ping_timeout=20):
        print("Local Nostr relay listening on ws://0.0.0.0:4848", flush=True)
        await asyncio.Future()


if __name__ == "__main__":
    asyncio.run(main())
