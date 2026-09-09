PRAGMA foreign_keys = ON;
CREATE TABLE accounts (
  id TEXT PRIMARY KEY,
  provider TEXT NOT NULL,
  subject TEXT NOT NULL,
  display_name TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  UNIQUE(provider, subject)
);
CREATE TABLE devices (
  id TEXT PRIMARY KEY,
  account_id TEXT NOT NULL REFERENCES accounts(id),
  token_hash TEXT NOT NULL UNIQUE,
  name TEXT NOT NULL,
  platform TEXT NOT NULL,
  address TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  last_seen INTEGER NOT NULL
);
CREATE INDEX devices_account ON devices(account_id);
CREATE TABLE challenges (
  id TEXT PRIMARY KEY,
  nonce_hash TEXT NOT NULL,
  expires INTEGER NOT NULL
);
CREATE INDEX challenges_expiry ON challenges(expires);
CREATE TABLE enrollments (
  id TEXT PRIMARY KEY,
  device_id TEXT NOT NULL,
  token_hash TEXT NOT NULL UNIQUE,
  name TEXT NOT NULL,
  platform TEXT NOT NULL,
  address TEXT NOT NULL,
  expires INTEGER NOT NULL
);
CREATE INDEX enrollments_expiry ON enrollments(expires);
CREATE TABLE oauth_states (
  state_hash TEXT PRIMARY KEY,
  enrollment_id TEXT NOT NULL REFERENCES enrollments(id) ON DELETE CASCADE,
  cookie_hash TEXT NOT NULL,
  verifier TEXT NOT NULL,
  expires INTEGER NOT NULL
);
CREATE TABLE confirmations (
  session_hash TEXT PRIMARY KEY,
  enrollment_id TEXT NOT NULL REFERENCES enrollments(id) ON DELETE CASCADE,
  subject TEXT NOT NULL,
  display_name TEXT NOT NULL,
  expires INTEGER NOT NULL
);
CREATE TABLE rate_limits (
  id TEXT PRIMARY KEY,
  count INTEGER NOT NULL,
  expires INTEGER NOT NULL
);
