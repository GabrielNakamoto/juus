# Juus - "Just Us"

P2P social group chat platform. Principles: low latency, privacy and features that people will use.

## Architecture chart

```mermaid
flowchart TD
    subgraph Frontend
        X(Model/Ui State) -->|render state| B(View/Ui)
        B -->|user input| X
    end
    subgraph Backend
        C(Event Handler) <-->|Message i/o| Y(P2P Node)
		C <-->|Peer discovery/node init| D[(Pubkey Registry)]
        C <-->|Offline message sync| F[(DHT Message Cache)]
    end
    X -->|Backend commands|C
    C -->|Backend event notifications|X
    Y <-->|Iroh P2P Protocl| E(Other group nodes)
```
