# 🎯 Système de Détection d'Objets IoT

Un système complet de détection d'objets par couleur avec interface web, backend Rust et intégration caméra temps réel.

## 🚀 Installation Rapide

### 1. Cloner et Organiser le Projet

```bash
mkdir detection_project
cd detection_project

# Créer la structure des dossiers
mkdir -p backend/src frontend detection
```

### 2. Copier les Fichiers

Copiez tous les fichiers fournis dans leurs dossiers respectifs selon cette structure :

```
detection_project/
├── backend/
│   ├── src/
│   │   ├── main.rs
│   │   ├── auth.rs
│   │   └── database.rs
│   └── Cargo.toml
├── frontend/
│   ├── index.html
│   ├── login.html
│   ├── history.html
│   ├── styles.css
│   └── scripts.js
├── detection/
│   ├── detection.py
│   └── requirements.txt
└── README.md
```

### 3. Installation des Dépendances

#### Python (Détection)

```bash
cd detection
python -m venv venv

# Windows
venv\Scripts\activate
# Linux/Mac
source venv/bin/activate

pip install -r requirements.txt
```

#### Rust (Backend)

```bash
# Installer Rust si pas déjà fait
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env

cd ../backend
cargo build --release
```

## 🏃‍♂️ Lancement de l'Application

### Méthode 1: Lancement Simple (Recommandé)

```bash
# 1. Démarrer le serveur backend (depuis le dossier backend/)
cd backend
cargo run

# 2. Ouvrir votre navigateur sur: http://localhost:3000
# L'application est maintenant accessible !
```

### Méthode 2: Lancement Manuel de la Détection

Si vous voulez tester la détection Python séparément :

```bash
# Dans un autre terminal
cd detection
source venv/bin/activate  # Linux/Mac
# ou venv\Scripts\activate  # Windows
python detection.py
```

## 🔐 Connexion

- **URL**: http://localhost:3000
- **Dashboard**: Accessible directement
- **Historique**: Nécessite une connexion
- **Identifiants par défaut**:
  - Username: `admin`
  - Password: `password123`

## 📊 Utilisation

### Dashboard Principal (`/`)

1. **Démarrer la Caméra**: Clic sur "Démarrer"
2. **Détection Automatique**: Objets détectés toutes les 1000ms
3. **Capture**: Prendre des screenshots d'objets détectés
4. **Compteurs**: Voir la cadence en temps réel
5. **Ajout Manuel**: Ajouter des objets via formulaire

### Page Historique (`/history.html`)

1. **Se connecter** avec admin/password123
2. **Filtrer** par date, type, couleur
3. **Télécharger** l'historique en TXT/CSV
4. **Ajouter** de nouveaux objets manuellement

## 🎨 Types d'Objets Détectés

| Couleur  | Type d'Objet        | Description       |
| -------- | ------------------- | ----------------- |
| 🔴 Rouge | Carte microchip     | Cartes avec puces |
| 🟢 Vert  | Carte personnalisée | Cartes custom     |
| 🔵 Bleu  | STM32               | Microcontrôleurs  |

## ⚙️ Configuration

### Modifier les Couleurs de Détection

Éditez `detection/detection.py` lignes 15-19 :

```python
colors = {
    'red': [(0, 120, 70), (10, 255, 255), (170, 120, 70), (180, 255, 255)],
    'green': [(36, 50, 70), (89, 255, 255)],
    'blue': [(90, 50, 70), (128, 255, 255)]
}
```

### Changer les Identifiants

Éditez `backend/src/auth.rs` lignes 6-7 :

```rust
const DEFAULT_USERNAME: &str = "votre_username";
const DEFAULT_PASSWORD: &str = "votre_password";
```

### Modifier le Port

Éditez `backend/src/main.rs` ligne 69 :

```rust
let listener = tokio::net::TcpListener::bind("0.0.0.0:VOTRE_PORT").await?;
```

## 🗄️ Base de Données

- **Type**: SQLite3
- **Fichier**: `backend/detection.db` (créé automatiquement)
- **Tables**:
  - `detections`: Détections individuelles
  - `daily_stats`: Statistiques journalières

### Structure des Données

#### Table `detections`:

```sql
- id: INTEGER PRIMARY KEY
- g_id: TEXT UNIQUE (ex: "RED_MICROCHIP_1699123456")
- ref_count: INTEGER (compteur G_ID*ref)
- type: TEXT ("Carte microchip", "Carte personnalisée", "STM32")
- color: TEXT ("red", "green", "blue")
- datetime: TEXT ("2024-01-15 10:30:00")
```

#### Table `daily_stats`:

```sql
- id: INTEGER PRIMARY KEY
- g_id: TEXT
- type: TEXT
- cadence: INTEGER (nombre de détections ce jour)
- day: TEXT ("2024-01-15")
```

## 🔧 Raccourcis Clavier

- `S`: Démarrer la caméra
- `Q`: Arrêter la caméra
- `C`: Capturer une frame
- `Ctrl+R`: Reset des compteurs
- `F12`: Afficher les infos de debug

## 🐛 Résolution des Problèmes

### Problème: Caméra non détectée

```bash
# Solutions:
1. Vérifier que la caméra est branchée
2. Donner l'autorisation au navigateur
3. Changer l'index caméra dans detection.py:
   cap = cv2.VideoCapture(1)  # au lieu de 0
```

### Problème: Erreur de compilation Rust

```bash
# Solutions:
1. Mettre à jour Rust:
   rustup update
2. Nettoyer le cache:
   cargo clean && cargo build
```

### Problème: Module Python non trouvé

```bash
# Solutions:
1. Activer l'environnement virtuel:
   source venv/bin/activate  # Linux/Mac
   venv\Scripts\activate     # Windows
2. Réinstaller les dépendances:
   pip install -r requirements.txt
```

### Problème: Port déjà utilisé

```bash
# Changer le port dans main.rs ou tuer le processus:
# Linux/Mac:
lsof -ti:3000 | xargs kill -9
# Windows:
netstat -ano | findstr :3000
taskkill /PID [PID_NUMBER] /F
```

### Problème: Base de données verrouillée

```bash
# Solutions:
1. Fermer toutes les instances de l'application
2. Supprimer detection.db pour recréer:
   rm backend/detection.db
```

## 📊 API Endpoints

### POST `/api/login`

```json
{
  "username": "admin",
  "password": "password123"
}
```

### POST `/api/detection`

```json
{
  "g_id": "RED_MICROCHIP_001",
  "object_type": "Carte microchip",
  "color": "red"
}
```

### GET `/api/history?from_date=2024-01-01&to_date=2024-01-31`

### GET `/api/stats`

### GET `/api/download?from_date=2024-01-01&to_date=2024-01-31`

## 🚀 Fonctionnalités Avancées

### 1. Détection en Temps Réel

- Traitement d'images avec OpenCV
- Détection de couleurs HSV
- Envoi automatique vers l'API
- Compteurs temps réel

### 2. Interface Web Moderne

- Design responsive
- Animations CSS
- Notifications temps réel
- Modals interactifs

### 3. Base de Données Robuste

- Triggers automatiques
- Vues optimisées
- Index pour performance
- Intégrité des données

### 4. Sécurité

- Authentification cryptée
- Sessions JWT (à implémenter)
- Validation des entrées
- Protection CORS

## 📈 Extensions Possibles

### Court Terme

- [ ] Amélioration des seuils de couleur
- [ ] Support multi-caméras
- [ ] Notifications par email
- [ ] Graphiques temps réel

### Long Terme

- [ ] Machine Learning (YOLO v8)
- [ ] API REST complète
- [ ] Interface mobile
- [ ] Cloud deployment
- [ ] Analyse prédictive

## 🔍 Debug et Logs

### Logs Backend (Rust)

```bash
# Démarrer avec logs détaillés
RUST_LOG=debug cargo run
```

### Logs Frontend (JavaScript)

```javascript
// Dans la console du navigateur
debugInfo(); // Affiche les informations système
```

### Logs Python

```bash
# Le script detection.py affiche automatiquement:
# - État des détections
# - Erreurs de connexion API
# - Statistiques FPS
```

## 📁 Architecture du Code

### Backend (Rust + Axum)

```
src/
├── main.rs      # Serveur principal, routes API
├── auth.rs      # Authentification et sécurité
└── database.rs  # Gestion SQLite, requêtes
```

### Frontend (HTML + CSS + JS)

```
├── index.html   # Dashboard principal
├── login.html   # Page de connexion
├── history.html # Historique et gestion
├── styles.css   # Design moderne, responsive
└── scripts.js   # Logique interface, API
```

### Détection (Python + OpenCV)

```
├── detection.py      # Script principal
└── requirements.txt  # Dépendances Python
```

## 🎯 Performance

### Optimisations Implémentées

- **Détection**: Traitement par blocs (20x20px)
- **Base**: Index sur colonnes critiques
- **Frontend**: Animations GPU, lazy loading
- **API**: Connexions persistantes, cache

### Métriques Typiques

- **FPS Caméra**: 30 FPS
- **Détection**: 1000ms (configurable)
- **Latence API**: < 100ms
- **Mémoire**: ~50MB total

## 📞 Support

### En cas de problème:

1. **Vérifier les logs** dans la console
2. **Tester chaque composant** séparément
3. **Vérifier les permissions** (caméra, fichiers)
4. **Redémarrer** tous les services

### Fichiers de log:

- Backend: Affiché dans le terminal
- Frontend: Console du navigateur (F12)
- Python: Terminal de detection.py

---

## ✅ Checklist de Vérification

Avant de signaler un problème, vérifiez :

- [ ] Python et Rust installés
- [ ] Dépendances installées (pip + cargo)
- [ ] Caméra connectée et fonctionnelle
- [ ] Port 3000 libre
- [ ] Permissions accordées au navigateur
- [ ] Environnement virtuel Python activé

---

**🎉 Votre système de détection IoT est maintenant opérationnel !**

Accédez à http://localhost:3000 et commencez à détecter vos objets !
