#!/usr/bin/env python3
"""Serve the docs desk: a section-by-section editor over every docs page.

Usage:
  desk.py serve [--host 127.0.0.1] [--port 4180] [--docs-url URL]
                [--preview-dir DIR]
  desk.py seed <seed.json>      create the pages and sections the desk lacks
  desk.py import <export dir>   load a dump, one JSON file per document
  desk.py export <out dir>      dump every collection for merge_docs.py

The store is SQLite at $DESK_DB, by default under $XDG_STATE_HOME. Every
document carries a version, and every write to an existing document must name
the version it read, so an agent never overwrites an edit it has not seen.

With --preview-dir, the desk mirrors the repository there: `docs/` is a copy,
and every other top-level entry links back to the checkout, because the site
imports examples from outside `docs/`. Each change is written into the copy
shortly after it lands. Run the docs site from `<preview-dir>/docs` to see
edits rendered before they reach the working tree.
"""

import argparse
from contextlib import closing
import json
import os
import pathlib
import shutil
import sqlite3
import subprocess
import sys
import tempfile
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import parse_qs, urlparse

HERE = pathlib.Path(__file__).resolve().parent
DOCS = HERE.parent
PREVIEW_SKIP = ("node_modules", "build", ".docusaurus", "tools")
PREVIEW_SETTLE = 0.4
PAGE = HERE / "desk.html"
COLLECTIONS = ("files", "sections", "facts")
SEED_SKIP = ("original", "originalText", "originalPreamble")
BATCH_LIMIT = 50
HISTORY = 1000

SCHEMA = """
create table if not exists docs (
    collection text not null,
    id text not null,
    version integer not null,
    data text not null,
    updated_at real not null,
    primary key (collection, id)
);
"""


def default_db():
    state = os.environ.get("XDG_STATE_HOME") or pathlib.Path.home() / ".local/state"
    return pathlib.Path(state) / "whisker-docs-desk" / "desk.db"


def connect(path):
    path.parent.mkdir(parents=True, exist_ok=True)
    db = sqlite3.connect(path, timeout=10, isolation_level=None)
    db.execute("pragma journal_mode = wal")
    db.execute("pragma busy_timeout = 10000")
    db.executescript(SCHEMA)
    return db


class Conflict(Exception):
    """A write named a version the document is no longer at."""

    def __init__(self, collection, id, current):
        super().__init__(f"{collection}/{id}")
        self.body = {
            "error": "version_mismatch",
            "collection": collection,
            "id": id,
            "current": current,
        }


class Invalid(Exception):
    pass


def row(collection, id, version, data):
    return {"collection": collection, "id": id, "version": version, "data": data}


def read(db, collection, id):
    found = db.execute(
        "select version, data from docs where collection = ? and id = ?",
        (collection, id),
    ).fetchone()
    return row(collection, id, found[0], json.loads(found[1])) if found else None


def apply(db, write):
    """Applies one write inside the caller's transaction.

    `put` replaces the data, `patch` merges top-level fields into it, and
    `delete` removes the document. Creating a document takes no version or
    version 0; anything that touches an existing one must name its version.
    """
    if not isinstance(write, dict):
        raise Invalid("each write must be an object")
    op = write.get("op", "put")
    collection, id = write.get("collection"), write.get("id")
    if collection not in COLLECTIONS or not isinstance(id, str) or not id:
        raise Invalid(f"bad target: {collection}/{id}")
    if op not in ("put", "patch", "delete"):
        raise Invalid(f"bad op: {op}")

    current = read(db, collection, id)
    expected = write.get("if_version")
    if current is None:
        if op != "put" or expected not in (None, 0):
            raise Conflict(collection, id, None)
    elif expected != current["version"]:
        raise Conflict(collection, id, current)

    if op == "delete":
        db.execute("delete from docs where collection = ? and id = ?", (collection, id))
        return {"collection": collection, "id": id, "version": None, "deleted": True}

    data = write.get("data")
    if not isinstance(data, dict):
        raise Invalid(f"data must be an object: {collection}/{id}")
    if op == "patch":
        data = {**current["data"], **data}
    version = current["version"] + 1 if current else 1
    db.execute(
        "insert or replace into docs values (?, ?, ?, ?, ?)",
        (collection, id, version, json.dumps(data), time.time()),
    )
    return row(collection, id, version, data)


def transact(db, writes):
    """Applies every write or none of them."""
    db.execute("begin immediate")
    try:
        results = [apply(db, write) for write in writes]
    except BaseException:
        db.execute("rollback")
        raise
    db.execute("commit")
    return results


def run_sync(db_path, *flags):
    """Runs sync_docs.sh against the store at db_path."""
    with tempfile.TemporaryDirectory(prefix="docs-desk-") as work:
        env = {
            **os.environ,
            "DESK_DB": str(db_path),
            "WORK": work,
            "PATH": f"{pathlib.Path(sys.executable).parent}:{os.environ.get('PATH', '')}",
        }
        return subprocess.run(
            ["bash", str(HERE / "sync_docs.sh"), *flags],
            env=env,
            capture_output=True,
            text=True,
            timeout=120,
        )


class Hub:
    """Fans each committed change out to every open event stream."""

    def __init__(self):
        self.lock = threading.Condition()
        self.seq = 0
        self.log = []

    def publish(self, changes):
        with self.lock:
            for change in changes:
                self.seq += 1
                self.log.append((self.seq, change))
            del self.log[:-HISTORY]
            self.lock.notify_all()

    def wait(self, after, timeout):
        with self.lock:
            if after > self.seq:
                return self.seq, None
            self.lock.wait_for(lambda: self.seq > after, timeout)
            if self.log and self.log[0][0] > after + 1:
                return self.seq, None
            return self.seq, [change for seq, change in self.log if seq > after]


class Handler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"
    db_path = None
    hub = None
    docs_url = None
    sync_lock = threading.Lock()

    def log_message(self, format, *args):
        if os.environ.get("DESK_LOG"):
            super().log_message(format, *args)

    def send(self, status, payload, kind="application/json"):
        body = payload if isinstance(payload, bytes) else json.dumps(payload).encode()
        self.send_response(status)
        self.send_header("Content-Type", kind)
        self.send_header("Content-Length", str(len(body)))
        self.send_header("Cache-Control", "no-store")
        self.end_headers()
        self.wfile.write(body)

    def body(self):
        length = int(self.headers.get("Content-Length") or 0)
        try:
            body = json.loads(self.rfile.read(length) or b"{}")
        except json.JSONDecodeError as error:
            raise Invalid(f"body is not JSON: {error}")
        if not isinstance(body, dict):
            raise Invalid("body must be an object")
        return body

    def route(self):
        url = urlparse(self.path)
        parts = [part for part in url.path.split("/") if part]
        return parts, parse_qs(url.query, keep_blank_values=True)

    def do_GET(self):
        parts, query = self.route()
        if not parts:
            return self.send(200, PAGE.read_bytes(), "text/html; charset=utf-8")
        if parts == ["api", "config"]:
            return self.send(200, {"docsUrl": self.docs_url})
        if parts == ["api", "events"]:
            after = query.get("after", ["0"])[0]
            return self.stream(int(after) if after.isdigit() else 0)
        with closing(connect(self.db_path)) as db:
            if parts == ["api"]:
                seq = self.hub.seq
                rows = db.execute(
                    "select collection, id, version, data from docs order by collection, id"
                ).fetchall()
                return self.send(
                    200,
                    {
                        "seq": seq,
                        "docs": [row(c, i, v, json.loads(d)) for c, i, v, d in rows],
                    },
                )
            if len(parts) == 2 and parts[0] == "api" and parts[1] in COLLECTIONS:
                rows = db.execute(
                    "select id, version, data from docs where collection = ? order by id",
                    (parts[1],),
                ).fetchall()
                return self.send(
                    200, [row(parts[1], i, v, json.loads(d)) for i, v, d in rows]
                )
            if len(parts) == 3 and parts[0] == "api":
                found = read(db, parts[1], parts[2])
                if found:
                    return self.send(200, found)
        self.send(404, {"error": "not_found"})

    def do_POST(self):
        parts, query = self.route()
        if parts == ["api", "sync"]:
            return self.sync("check" in query)
        if parts != ["api", "batch"]:
            return self.send(404, {"error": "not_found"})
        try:
            writes = self.body().get("writes") or []
            if len(writes) > BATCH_LIMIT:
                raise Invalid(f"a batch holds at most {BATCH_LIMIT} writes")
            with closing(connect(self.db_path)) as db:
                results = transact(db, writes)
        except Conflict as conflict:
            return self.send(409, conflict.body)
        except Invalid as error:
            return self.send(400, {"error": "invalid", "detail": str(error)})
        self.hub.publish(results)
        self.send(200, {"results": results})

    def sync(self, check):
        """Runs sync_docs.sh against this store, one sync at a time."""
        if not self.sync_lock.acquire(blocking=False):
            return self.send(409, {"error": "sync_running"})
        try:
            done = run_sync(self.db_path, *(["--check"] if check else []))
        except subprocess.TimeoutExpired:
            return self.send(504, {"error": "sync_timed_out"})
        finally:
            self.sync_lock.release()
        self.send(
            200,
            {"code": done.returncode, "stdout": done.stdout, "stderr": done.stderr},
        )

    def stream(self, after):
        """Sends each change as a server-sent event, and a ping when idle."""
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.send_header("Cache-Control", "no-store")
        self.send_header("Connection", "close")
        self.end_headers()
        self.close_connection = True
        try:
            while True:
                seq, changes = self.hub.wait(after, 20)
                if changes is None:
                    self.wfile.write(f"event: reset\ndata: {seq}\n\n".encode())
                elif changes:
                    payload = json.dumps({"seq": seq, "changes": changes})
                    self.wfile.write(f"data: {payload}\n\n".encode())
                else:
                    self.wfile.write(b": ping\n\n")
                self.wfile.flush()
                after = seq
        except (BrokenPipeError, ConnectionResetError):
            pass


def preview(hub, db_path, mirror):
    """Keeps the copy at mirror/docs equal to the tree plus the desk's edits.

    Each burst of changes gets one render, after the burst settles, so typing
    does not queue a render per keystroke.
    """
    mirror.mkdir(parents=True, exist_ok=True)
    for entry in DOCS.parent.iterdir():
        link = mirror / entry.name
        if entry.name not in (DOCS.name, ".git") and not link.is_symlink():
            link.symlink_to(entry)
    target = mirror / DOCS.name
    shutil.copytree(
        DOCS, target, dirs_exist_ok=True, ignore=shutil.ignore_patterns(*PREVIEW_SKIP)
    )
    modules = target / "node_modules"
    if not modules.exists():
        modules.symlink_to(DOCS / "node_modules")

    after = hub.seq
    while True:
        try:
            done = run_sync(db_path, f"--out={target}")
            if done.returncode:
                print(f"preview render failed:\n{done.stderr}", file=sys.stderr)
        except subprocess.TimeoutExpired:
            print("preview render timed out", file=sys.stderr)
        seq = after
        while seq == after:
            seq, _ = hub.wait(after, 3600)
        time.sleep(PREVIEW_SETTLE)
        after = hub.seq


def serve(args, db_path):
    connect(db_path).close()
    Handler.db_path = db_path
    Handler.hub = Hub()
    Handler.docs_url = args.docs_url
    if args.preview_dir:
        threading.Thread(
            target=preview,
            args=(Handler.hub, db_path, args.preview_dir.resolve()),
            daemon=True,
        ).start()
    server = ThreadingHTTPServer((args.host, args.port), Handler)
    server.daemon_threads = True
    print(f"desk on http://{args.host}:{args.port}, store {db_path}", file=sys.stderr)
    server.serve_forever()


def seed(args, db_path):
    payload = json.loads(pathlib.Path(args.seed).read_text())
    created = 0
    with closing(connect(db_path)) as db:
        db.execute("begin immediate")
        for collection in ("files", "sections"):
            for record in payload[collection]:
                if read(db, collection, record["id"]):
                    continue
                data = {k: v for k, v in record.items() if k not in SEED_SKIP}
                apply(db, {"collection": collection, "id": record["id"], "data": data})
                created += 1
        db.execute("commit")
    print(f"{created} documents created", file=sys.stderr)


def import_dump(args, db_path):
    """Loads a dump, keeping each document's version when the dump has one."""
    root = pathlib.Path(args.dir)
    loaded = 0
    with closing(connect(db_path)) as db:
        db.execute("begin immediate")
        for collection in COLLECTIONS:
            for path in sorted((root / collection).glob("*.json")):
                payload = json.loads(path.read_text())
                data = payload.get("data", payload)
                id = payload.get("id") or data.get("id") or path.stem
                version = payload.get("version") or 1
                db.execute(
                    "insert or replace into docs values (?, ?, ?, ?, ?)",
                    (collection, id, version, json.dumps(data), time.time()),
                )
                loaded += 1
        db.execute("commit")
    print(f"{loaded} documents imported", file=sys.stderr)


def export(args, db_path):
    root = pathlib.Path(args.dir)
    with closing(connect(db_path)) as db:
        rows = db.execute("select collection, id, version, data from docs").fetchall()
    for collection in COLLECTIONS:
        target = root / collection
        target.mkdir(parents=True, exist_ok=True)
        for stale in target.glob("*.json"):
            stale.unlink()
    for collection, id, version, data in rows:
        path = root / collection / f"{id}.json"
        path.write_text(json.dumps(row(collection, id, version, json.loads(data))))
    print(f"{len(rows)} documents exported to {root}", file=sys.stderr)


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--db", type=pathlib.Path)
    commands = parser.add_subparsers(dest="command", required=True)
    serving = commands.add_parser("serve")
    serving.add_argument("--host", default="127.0.0.1")
    serving.add_argument("--port", type=int, default=4180)
    serving.add_argument("--docs-url", help="the running docs site, for previews")
    serving.add_argument("--preview-dir", type=pathlib.Path, help="where to render edits")
    commands.add_parser("seed").add_argument("seed")
    commands.add_parser("import").add_argument("dir")
    commands.add_parser("export").add_argument("dir")
    args = parser.parse_args()

    db_path = args.db or pathlib.Path(os.environ.get("DESK_DB") or default_db())
    {"serve": serve, "seed": seed, "import": import_dump, "export": export}[
        args.command
    ](args, db_path)


main()
