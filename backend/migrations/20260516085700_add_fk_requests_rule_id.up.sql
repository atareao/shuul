ALTER TABLE requests
    ADD CONSTRAINT fk_requests_rule_id
    FOREIGN KEY (rule_id)
    REFERENCES rules(id)
    ON DELETE SET NULL;
