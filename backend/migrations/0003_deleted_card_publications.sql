-- Retain only anonymous publication identities to cancel already subscribed events.
ALTER TABLE calendar_publications DROP CONSTRAINT calendar_publications_event_id_fkey;
