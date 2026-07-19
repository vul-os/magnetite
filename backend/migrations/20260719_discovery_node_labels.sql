-- Node-declared labels on discovery ads (DECENTRALIZATION.md §3.4).
--
-- `operator` and `region` are things a tracker CANNOT verify. A phonebook has
-- no way to confirm that a box in Frankfurt is really in Frankfurt, or that the
-- person running it is really "gridhost". So these are carried INSIDE the
-- signed `SessionAd` body: they are bound to the node key, which means a relay
-- or a malicious tracker cannot relabel someone else's box — but they remain a
-- CLAIM, not a certification. The UI must present them as node-declared.
--
-- Both are nullable: a node that declares nothing is normal, and the surface
-- must degrade to showing just the address.

ALTER TABLE discovery_ads
    ADD COLUMN IF NOT EXISTS operator TEXT,
    ADD COLUMN IF NOT EXISTS region   TEXT;
