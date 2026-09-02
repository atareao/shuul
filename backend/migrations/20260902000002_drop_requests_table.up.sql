-- Drop the requests table since we no longer store individual requests
-- Stats are now kept in memory via StatsCollector and persisted to stats_cache
DROP TABLE IF EXISTS requests CASCADE;