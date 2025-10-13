Central server to run:
- Pubkey registry
- DHT Message Cache bootstrap node (kademlia_dht)
- P2P iroh bootstrap node?

Each back end runs:
- kademlia DHT node for the offline message cache
- P2P iroh node for group communication
- Sqlite db for group message history

* How should a groups messages be stored?
	* Start with local db (sqlite most likely)
	* Add sharding for scalability if needed
