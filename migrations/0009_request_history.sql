CREATE INDEX requests_created_at ON requests (created_at);

-- Do not reuse an ID after pruning; it can still appear in an open action form.
CREATE TABLE request_id_sequence (last_id INTEGER NOT NULL);
INSERT INTO request_id_sequence SELECT COALESCE(MAX(id), 0) FROM requests;
CREATE TRIGGER requests_advance_sequence AFTER INSERT ON requests
BEGIN
    UPDATE request_id_sequence SET last_id = MAX(last_id, NEW.id);
END;
