### Überblick über die App: Super-Terminal

Die "Super-Terminal" ist eine hochmodulare, erweiterbare Terminal-Anwendung, die als hybrider Terminal-Emulator und Multiplexer konzipiert ist. Sie orientiert sich stark an Ghostty als performantem, GPU-beschleunigtem Terminal-Emulator und Zellij als intuitivem Terminal-Workspace mit Pane-Management und Plugin-System. Das Ziel ist es, einen "Terminal Desktop" zu schaffen – eine Umgebung, in der der Benutzer mehrere Terminal-Fenster (Panes) frei verschieben, skalieren, gruppieren oder unabhängig voneinander nutzen kann, ähnlich wie in einem grafischen Desktop. Die App läuft nativ auf Windows und Linux, mit Fokus auf Geschwindigkeit, Stabilität und Erweiterbarkeit durch Plugins. 

Im Kern kombiniert sie die Emulationsstärke von Ghostty (z. B. standards-konforme Terminal-Sequenzen, GPU-Rendering für flüssige Darstellung) mit dem Multiplexing von Zellij (z. B. Sessions, Panes und kollaborative Features). Der einzigartige Twist: Ein offenes Plugin-Ökosystem, das den Terminal zu einem vielseitigen Tool macht, das über klassische Shell-Befehle hinausgeht – von Netzwerk-Tools bis hin zu Blockchain-Integrationen.

### Ziele und Anforderungen
- **Zielgruppe**: Entwickler, SysAdmins, Power-User und Enthusiasten, die eine zentrale App für Terminal-Arbeit, Netzwerk-Management und erweiterte Funktionen brauchen.
- **Plattform-Support**: 
  - Windows (via WinAPI oder Cross-Platform-Bibliotheken wie crossterm).
  - Linux (via GTK oder direkte TTY-Integration).
  - Kein macOS-Support, um den Fokus auf die angeforderten Plattformen zu legen.
- **Leitprinzipien**: 
  - Hohe Performance: GPU-Acceleration für Rendering, dedizierte Threads für I/O.
  - Modularität: Kernfunktionen minimal, Erweiterungen via Plugins.
  - Benutzerfreundlichkeit: Intuitive Steuerung (Tastenkürzel, Maus-Support), ohne Überladung.
  - Sicherheit: Plugins in Sandbox (z. B. WebAssembly) ausführen, um Risiken zu minimieren.
- **Nicht-Ziele**: Keine vollständige GUI-Desktop-Ersetzung; bleibt textbasiert, aber mit visuellen Hilfen wie Splits und Overlays.

### Kernfeatures
Basierend auf Ghostty und Zellij integriert die App folgende Basisfunktionen:
- **Terminal-Emulation**: Vollständige Unterstützung für ANSI/ECMA-48-Sequenzen, Ligaturen, Farben und Unicode. GPU-Rendering (OpenGL auf Linux, DirectX auf Windows) für niedrige Latenz, ähnlich Ghostty. Dedizierter I/O-Thread, um Jitter bei hoher Last zu vermeiden.
- **Pane-Management (Terminal Desktop)**: 
  - Frei verschiebbare Panes: Benutzer können Panes drag-and-drop verschieben, resizen oder stapeln (stacked/floating wie in Zellij).
  - Separat oder simultan nutzen: Panes können unabhängig laufen (z. B. ein Pane für SSH, eines für Code-Editing) oder synchronisiert werden (z. B. Broadcast-Modus für Befehle in mehreren Panes).
  - Sessions: Persistente Sessions, die bei Neustart wiederhergestellt werden können, mit Multi-Client-Support für Kollaboration.
- **UI-Elemente**: Tab-Bar, Split-Screens, Maus-Interaktion für Resizing. Konfigurierbar via YAML- oder TOML-Dateien.
- **Integrierte Tools**: Basis-Shell-Integration (Bash, Zsh, PowerShell auf Windows), Suchfunktion über Panes hinweg und Crash-Reporting.

### Plugin-System
Das Herzstück der App: Ein flexibles, herunterladbares Plugin-System, inspiriert von Zellijs WebAssembly-Plugins. Plugins erweitern die App dynamisch, ohne Neukompilierung. 

- **Wie es funktioniert**:
  - **Installation**: Plugins als WASM-Module herunterladen (z. B. von einem zentralen Repository wie GitHub oder einem dedizierten Store). Einfaches Hinzufügen via Befehl: `super-terminal plugin add <url oder name>`.
  - **Integration**: Plugins laden sich in dedizierte Panes oder Overlays. Sie können auf App-APIs zugreifen (z. B. für I/O, Netzwerk oder UI-Elemente), aber in einer Sandbox (WebAssembly) laufen, um Sicherheit zu gewährleisten.
  - **Entwicklung**: Plugins in beliebigen Sprachen (Rust, JS, etc.), die zu WASM kompilieren. API für Hooks (z. B. on-load, on-command).
  - **Management**: Plugin-Manager im Terminal: Liste, Update, Deinstallieren. Automatische Updates optional.

- **Beispiel-Plugins** (basierend auf deinen Vorschlägen; erweiterbar):
  | Plugin-Name | Beschreibung | Funktionen | Integration |
  |-------------|--------------|------------|-------------|
  | Bluetooth Messaging | Ermöglicht Messaging über Bluetooth-Geräte. | Scannen/Verbinden mit Geräten, Senden/Empfangen von Nachrichten, Datei-Übertragung. | Pane für Geräte-Liste; nutzt OS-Bluetooth-APIs (z. B. BlueZ auf Linux, Windows Bluetooth API). |
  | Mesh & WLAN Messaging | Peer-to-Peer-Messaging über Mesh-Netzwerke oder WLAN. | Erstellen/Beitreten von Meshes, verschlüsselte Chats, Offline-Messaging. | Integriert mit Netzwerk-Stack; Pane für Chat-Interface. |
  | Midnight Blockchain Integration | Wallet und private Transaktionen auf der Midnight-Sidechain (Cardano-basiert mit ZK-Proofs für Privacy). | Wallet-Management, private Überweisungen, Dust-Handling (kleine Beträge), NIGHT-Token-Support. | Sichere Key-Storage; Pane für Transaktions-Übersicht, Integration mit Midnight-API für ZK-SNARKs. |
  | Text/Code Editor | Eingebauter Editor für Dateien. | Syntax-Highlighting, Auto-Complete, Multi-File-Editing. | Vim/Emacs-ähnlich, aber in Pane integriert; nutzt Tree-Sitter für Parsing. |
  | File Sharing | Sicheres Teilen von Dateien. | Upload/Download via Links, P2P-Übertragung, Verschlüsselung. | Drag-and-Drop in Panes; Integration mit IPFS oder ähnlich. |
  | Network Tools | Suite für Netzwerk-Diagnose. | Ping, Traceroute, Port-Scan, WiFi-Analyse. | Kommando-basiert, mit visuellen Graphs in Panes. |
  | SSH Tools | Erweiterte SSH-Management. | Multi-Session-SSH, Key-Management, Tunneling. | Automatische Verbindungen in separaten Panes. |
  | Converter | Universal-Konverter. | Währungen, Einheiten, Dateiformate, Crypto-Conversions. | Interaktives Pane; erweiterbar via Sub-Plugins. |

Diese Plugins machen den Terminal zu einem "Super-Tool", das über reine Kommando-Ausführung hinausgeht – z. B. ein Plugin könnte ein Pane in einen Chat-Client oder Wallet verwandeln.

### Architektur
- **Sprache und Frameworks**: Rust als Kernsprache (wie Zellij), für Cross-Platform-Support und Sicherheit. Bibliotheken: crossterm für TTY, wgpu für GPU-Rendering, wasmtime für WASM-Plugins.
- **Modulare Struktur**:
  - **Core**: Terminal-Emulator (inspiriert von Ghosttys libghostty), handhabt Parsing, Rendering und I/O.
  - **Multiplexer**: Pane- und Session-Manager (ähnlich Zellij), mit Event-Loop für Interaktionen.
  - **Plugin-Engine**: WASM-Runtime, API-Exposer.
  - **Platform-Layer**: Abstraktion für Windows (WinAPI) und Linux (GTK/TTY).
- **Datenfluss**: Zentrale Event-Loop verarbeitet Eingaben, rendert Panes parallel und delegiert an Plugins.
- **Sicherheit**: Plugins isoliert, keine direkten OS-Zugriffe ohne Erlaubnis; ZK-Proofs in Midnight-Plugin für Privacy.

### Entwicklungsschritte
1. **Prototyping**: Basis-Emulator in Rust implementieren, mit einfachem Pane-Splitting.
2. **Plattform-Portierung**: Linux zuerst (einfacher), dann Windows-Integration.
3. **Plugin-System**: WASM-Integration hinzufügen, mit Beispiel-Plugins.
4. **Features erweitern**: Bluetooth/WLAN via OS-APIs, Midnight via Cardano-SDK.
5. **Testing**: Unit-Tests für Emulation, Integration-Tests für Plugins; Cross-Platform-Builds mit CI (GitHub Actions).
6. **Release**: Open-Source auf GitHub, Binaries für Windows/Linux.

### Potenzielle Challenges und Lösungen
- **Cross-Platform**: Unterschiede in APIs (z. B. Bluetooth) – Lösen durch Abstraktions-Layer (z. B. rust-bluetooth-Crate).
- **Performance**: GPU auf Windows – Nutze wgpu für einheitliches Rendering.
- **Sicherheit bei Plugins**: WASM-Sandboxing verhindert Missbrauch; User-Confirm für sensible Zugriffe.
- **Blockchain-Integration**: Midnight ist privacy-fokussiert mit ZK-SNARKs; Stelle sichere Wallet-Handling sicher, ohne Keys zu exponieren.
- **Community**: Fördere Plugin-Entwicklung durch Docs und ein Repo für Beiträge.

Diese Planung bietet eine solide Basis – bei Bedarf kann ich Details zu einem bestimmten Aspekt vertiefen!

### Build-Kommandos (aktueller Stand)
- **App bauen (default):** `cargo build -p uterx`
- **Plugins (WASM) bauen:** `cargo run -p xtask -- build-plugins`
- **App + Plugins kombiniert:** `cargo run -p xtask -- build-all`

Alias-Schreibweisen funktionieren ebenfalls:
- `cargo run -p xtask -- build plugins` → wie `build-plugins`
- `cargo run -p xtask -- build all` → wie `build-all`

Hinweis: Für Plugin-WASM-Builds muss das Target `wasm32-wasip1` installiert sein (`rustup target add wasm32-wasip1`).