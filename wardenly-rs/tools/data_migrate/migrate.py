#!/usr/bin/env python3
"""
Universal Data Migration Tool for wardenly-rs

Supports bidirectional migration between SQLite and MongoDB.

Usage examples:
    # SQLite -> MongoDB
    python migrate.py \\
        --source-type sqlite --source-path /path/to/wardenly.db \\
        --target-type mongodb --target-uri mongodb://localhost:27017 --target-db wardenly

    # MongoDB -> SQLite
    python migrate.py \\
        --source-type mongodb --source-uri mongodb://localhost:27017 --source-db wardenly \\
        --target-type sqlite --target-path /path/to/wardenly.db

    # MongoDB -> MongoDB
    python migrate.py \\
        --source-type mongodb --source-uri mongodb://localhost:27017 --source-db wardenly_old \\
        --target-type mongodb --target-uri mongodb://localhost:27017 --target-db wardenly_new

    # SQLite -> SQLite
    python migrate.py \\
        --source-type sqlite --source-path /path/to/old.db \\
        --target-type sqlite --target-path /path/to/new.db

Options:
    --dry-run       Preview changes without writing
    --skip-cookies  Exclude cookies field from migration
"""

import argparse
import json
import sqlite3
from abc import ABC, abstractmethod
from dataclasses import dataclass
from datetime import datetime
from pathlib import Path
from typing import Any

from pymongo import MongoClient

# ============================================================================
# DATA MODELS
# ============================================================================


@dataclass
class Account:
    id: str
    role_name: str
    user_name: str
    password: str
    server_id: int
    ranking: int
    cookies: list[dict[str, Any]] | None = None


@dataclass
class Group:
    id: str
    name: str
    description: str | None
    account_ids: list[str]
    ranking: int


# ============================================================================
# STORAGE BACKENDS
# ============================================================================


class StorageBackend(ABC):
    """Abstract base class for storage backends."""

    @abstractmethod
    def connect(self) -> None:
        """Establish connection to the storage."""
        pass

    @abstractmethod
    def close(self) -> None:
        """Close the connection."""
        pass

    @abstractmethod
    def read_accounts(self) -> list[Account]:
        """Read all accounts from storage."""
        pass

    @abstractmethod
    def read_groups(self) -> list[Group]:
        """Read all groups from storage."""
        pass

    @abstractmethod
    def write_accounts(self, accounts: list[Account]) -> None:
        """Write accounts to storage."""
        pass

    @abstractmethod
    def write_groups(self, groups: list[Group]) -> None:
        """Write groups to storage."""
        pass

    @abstractmethod
    def ensure_schema(self) -> None:
        """Ensure the storage schema exists."""
        pass


class SqliteBackend(StorageBackend):
    """SQLite storage backend."""

    def __init__(self, path: str):
        self.path = Path(path)
        self.conn: sqlite3.Connection | None = None

    def connect(self) -> None:
        # Ensure parent directory exists
        self.path.parent.mkdir(parents=True, exist_ok=True)
        self.conn = sqlite3.connect(str(self.path))
        self.conn.row_factory = sqlite3.Row

    def close(self) -> None:
        if self.conn:
            self.conn.close()
            self.conn = None

    def ensure_schema(self) -> None:
        if not self.conn:
            raise RuntimeError("Not connected")

        self.conn.execute("""
            CREATE TABLE IF NOT EXISTS accounts (
                id TEXT PRIMARY KEY,
                role_name TEXT NOT NULL,
                user_name TEXT NOT NULL,
                password TEXT NOT NULL,
                server_id INTEGER NOT NULL,
                ranking INTEGER DEFAULT 0,
                cookies TEXT
            )
        """)

        self.conn.execute("""
            CREATE TABLE IF NOT EXISTS groups (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                description TEXT,
                account_ids TEXT NOT NULL,
                ranking INTEGER DEFAULT 0
            )
        """)

        self.conn.commit()

    def read_accounts(self) -> list[Account]:
        if not self.conn:
            raise RuntimeError("Not connected")

        cursor = self.conn.execute(
            "SELECT id, role_name, user_name, password, server_id, ranking, cookies "
            "FROM accounts ORDER BY id"
        )

        accounts = []
        for row in cursor:
            cookies = None
            if row["cookies"]:
                try:
                    cookies = json.loads(row["cookies"])
                except json.JSONDecodeError:
                    pass

            accounts.append(Account(
                id=row["id"],
                role_name=row["role_name"],
                user_name=row["user_name"],
                password=row["password"],
                server_id=row["server_id"],
                ranking=row["ranking"],
                cookies=cookies,
            ))

        return accounts

    def read_groups(self) -> list[Group]:
        if not self.conn:
            raise RuntimeError("Not connected")

        cursor = self.conn.execute(
            "SELECT id, name, description, account_ids, ranking "
            "FROM groups ORDER BY id"
        )

        groups = []
        for row in cursor:
            account_ids = []
            if row["account_ids"]:
                try:
                    account_ids = json.loads(row["account_ids"])
                except json.JSONDecodeError:
                    pass

            groups.append(Group(
                id=row["id"],
                name=row["name"],
                description=row["description"],
                account_ids=account_ids,
                ranking=row["ranking"],
            ))

        return groups

    def write_accounts(self, accounts: list[Account]) -> None:
        if not self.conn:
            raise RuntimeError("Not connected")

        for acc in accounts:
            cookies_json = json.dumps(acc.cookies) if acc.cookies else None
            self.conn.execute(
                "INSERT OR REPLACE INTO accounts "
                "(id, role_name, user_name, password, server_id, ranking, cookies) "
                "VALUES (?, ?, ?, ?, ?, ?, ?)",
                (acc.id, acc.role_name, acc.user_name, acc.password,
                 acc.server_id, acc.ranking, cookies_json),
            )

        self.conn.commit()

    def write_groups(self, groups: list[Group]) -> None:
        if not self.conn:
            raise RuntimeError("Not connected")

        for grp in groups:
            account_ids_json = json.dumps(grp.account_ids)
            self.conn.execute(
                "INSERT OR REPLACE INTO groups "
                "(id, name, description, account_ids, ranking) "
                "VALUES (?, ?, ?, ?, ?)",
                (grp.id, grp.name, grp.description, account_ids_json, grp.ranking),
            )

        self.conn.commit()


class MongoBackend(StorageBackend):
    """MongoDB storage backend."""

    def __init__(self, uri: str, database: str):
        self.uri = uri
        self.database_name = database
        self.client: MongoClient | None = None
        self.db: Any = None

    def connect(self) -> None:
        self.client = MongoClient(self.uri)
        self.db = self.client[self.database_name]
        # Test connection
        self.client.admin.command("ping")

    def close(self) -> None:
        if self.client:
            self.client.close()
            self.client = None
            self.db = None

    def ensure_schema(self) -> None:
        # MongoDB doesn't require explicit schema creation
        pass

    def read_accounts(self) -> list[Account]:
        if not self.db:
            raise RuntimeError("Not connected")

        cursor = self.db.accounts.find().sort("_id", 1)

        accounts = []
        for doc in cursor:
            accounts.append(Account(
                id=doc["_id"],
                role_name=doc.get("role_name", ""),
                user_name=doc.get("user_name", ""),
                password=doc.get("password", ""),
                server_id=doc.get("server_id", 0),
                ranking=doc.get("ranking", 0),
                cookies=doc.get("cookies"),
            ))

        return accounts

    def read_groups(self) -> list[Group]:
        if not self.db:
            raise RuntimeError("Not connected")

        cursor = self.db.groups.find().sort("_id", 1)

        groups = []
        for doc in cursor:
            groups.append(Group(
                id=doc["_id"],
                name=doc.get("name", ""),
                description=doc.get("description"),
                account_ids=doc.get("account_ids", []),
                ranking=doc.get("ranking", 0),
            ))

        return groups

    def write_accounts(self, accounts: list[Account]) -> None:
        if not self.db:
            raise RuntimeError("Not connected")

        for acc in accounts:
            doc = {
                "_id": acc.id,
                "role_name": acc.role_name,
                "user_name": acc.user_name,
                "password": acc.password,
                "server_id": acc.server_id,
                "ranking": acc.ranking,
            }
            if acc.cookies is not None:
                doc["cookies"] = acc.cookies

            self.db.accounts.replace_one({"_id": acc.id}, doc, upsert=True)

    def write_groups(self, groups: list[Group]) -> None:
        if not self.db:
            raise RuntimeError("Not connected")

        for grp in groups:
            doc = {
                "_id": grp.id,
                "name": grp.name,
                "description": grp.description,
                "account_ids": grp.account_ids,
                "ranking": grp.ranking,
            }

            self.db.groups.replace_one({"_id": grp.id}, doc, upsert=True)


# ============================================================================
# MIGRATION LOGIC
# ============================================================================


def create_backend(
    storage_type: str,
    path: str | None = None,
    uri: str | None = None,
    database: str | None = None,
) -> StorageBackend:
    """Create a storage backend based on type."""
    if storage_type == "sqlite":
        if not path:
            raise ValueError("SQLite requires --source-path or --target-path")
        return SqliteBackend(path)
    elif storage_type == "mongodb":
        if not uri or not database:
            raise ValueError("MongoDB requires --source-uri/--target-uri and --source-db/--target-db")
        return MongoBackend(uri, database)
    else:
        raise ValueError(f"Unknown storage type: {storage_type}")


def migrate(
    source: StorageBackend,
    target: StorageBackend,
    dry_run: bool = False,
    skip_cookies: bool = False,
) -> None:
    """Perform the migration."""
    print("\n--- Reading from source ---")

    accounts = source.read_accounts()
    print(f"Found {len(accounts)} accounts")

    groups = source.read_groups()
    print(f"Found {len(groups)} groups")

    # Optionally strip cookies
    if skip_cookies:
        print("Stripping cookies from accounts...")
        for acc in accounts:
            acc.cookies = None

    # Log details
    for acc in accounts:
        cookies_info = f", {len(acc.cookies)} cookies" if acc.cookies else ""
        print(f"  Account: {acc.id} ({acc.role_name}{cookies_info})")

    for grp in groups:
        print(f"  Group: {grp.id} ({grp.name}, {len(grp.account_ids)} accounts)")

    if dry_run:
        print("\n[DRY-RUN] Would write to target:")
        print(f"  - {len(accounts)} accounts")
        print(f"  - {len(groups)} groups")
        return

    print("\n--- Writing to target ---")

    target.ensure_schema()

    target.write_accounts(accounts)
    print(f"✓ Wrote {len(accounts)} accounts")

    target.write_groups(groups)
    print(f"✓ Wrote {len(groups)} groups")


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Universal data migration tool for wardenly-rs",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=__doc__,
    )

    # Source arguments
    parser.add_argument("--source-type", required=True, choices=["sqlite", "mongodb"],
                        help="Source storage type")
    parser.add_argument("--source-path", help="Source SQLite file path")
    parser.add_argument("--source-uri", help="Source MongoDB URI")
    parser.add_argument("--source-db", help="Source MongoDB database name")

    # Target arguments
    parser.add_argument("--target-type", required=True, choices=["sqlite", "mongodb"],
                        help="Target storage type")
    parser.add_argument("--target-path", help="Target SQLite file path")
    parser.add_argument("--target-uri", help="Target MongoDB URI")
    parser.add_argument("--target-db", help="Target MongoDB database name")

    # Options
    parser.add_argument("--dry-run", action="store_true",
                        help="Preview changes without writing")
    parser.add_argument("--skip-cookies", action="store_true",
                        help="Exclude cookies field from migration")

    args = parser.parse_args()

    print("=" * 60)
    print("Data Migration Tool for wardenly-rs")
    print("=" * 60)
    print(f"Source: {args.source_type}")
    print(f"Target: {args.target_type}")
    print(f"Mode: {'DRY-RUN' if args.dry_run else 'EXECUTE'}")
    print(f"Skip cookies: {args.skip_cookies}")
    print(f"Started at: {datetime.now().isoformat()}")
    print("=" * 60)

    # Create backends
    source = create_backend(
        args.source_type,
        path=args.source_path,
        uri=args.source_uri,
        database=args.source_db,
    )

    target = create_backend(
        args.target_type,
        path=args.target_path,
        uri=args.target_uri,
        database=args.target_db,
    )

    try:
        print("\nConnecting to source...")
        source.connect()
        print("✓ Source connected")

        print("Connecting to target...")
        target.connect()
        print("✓ Target connected")

        if not args.dry_run:
            confirm = input("\nThis will write to the target. Continue? [y/N] ")
            if confirm.lower() != "y":
                print("Aborted.")
                return

        migrate(source, target, dry_run=args.dry_run, skip_cookies=args.skip_cookies)

        print("\n" + "=" * 60)
        print("Migration completed successfully!")
        print("=" * 60)

    finally:
        source.close()
        target.close()


if __name__ == "__main__":
    main()
