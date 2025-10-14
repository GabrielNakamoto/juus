**Central server runs:**
- Pubkey registry
- DHT Message Cache bootstrap node (kademlia_dht)
- P2P iroh bootstrap node?

**Juus backend runs:**
- kademlia DHT node for the offline message cache
- P2P iroh node for group communication
- Sqlite db for group message history

**Design problems:**
* How should a groups messages be stored?
	* Start with local db (sqlite most likely)
	* Add sharding for scalability if needed
	* What does sql message storage look like?
* How should the project be seperated/organized? (cargo workspace?)
	* Packages and crates (depth respective):
		* Backend
			* Event handler
			* P2P node
			* Db connections
		* Frontend
			* Model / Ui state
			* UI (components)
			* User input handling
* How to know group members?
	* Store names and public keys in local sqlite db
* But there still needs to be a way to join the group?
	* Have the pubkey registry also store group (group name, Vec<username>) pairs, pubkeys can be queried after
