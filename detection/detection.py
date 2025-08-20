#!/usr/bin/env python3
"""
Script Python pour envoyer des détections au système IoT
Simule un capteur intelligent qui envoie des données de détection
"""

import requests
import json
import time
import random
from datetime import datetime
from typing import Dict, Any, Optional
import logging

# Configuration
API_BASE_URL = "http://localhost:3000/api"
DEFAULT_USERNAME = "admin"
DEFAULT_PASSWORD = "Admin123!"

# Configuration du logging
logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(levelname)s - %(message)s',
    handlers=[
        logging.FileHandler('detection_sender.log'),
        logging.StreamHandler()
    ]
)
logger = logging.getLogger(__name__)

class DetectionSender:
    def __init__(self, base_url: str = API_BASE_URL):
        self.base_url = base_url
        self.session = requests.Session()
        self.auth_token: Optional[str] = None
        
    def login(self, username: str = DEFAULT_USERNAME, password: str = DEFAULT_PASSWORD) -> bool:
        """Authentification sur l'API"""
        login_data = {
            "username": username,
            "password": password
        }
        
        try:
            response = self.session.post(
                f"{self.base_url}/login",
                json=login_data,
                timeout=10
            )
            
            if response.status_code == 200:
                data = response.json()
                if data.get("success"):
                    self.auth_token = data["data"]["token"]
                    self.session.headers.update({
                        "Authorization": f"Bearer {self.auth_token}"
                    })
                    logger.info(f"✅ Connexion réussie pour {username}")
                    return True
                else:
                    logger.error(f"❌ Échec de connexion: {data.get('message')}")
            else:
                logger.error(f"❌ Erreur HTTP {response.status_code}: {response.text}")
                
        except requests.exceptions.RequestException as e:
            logger.error(f"❌ Erreur de connexion: {e}")
            
        return False
    
    def send_detection(self, g_id: str, object_type: str, color: str, confidence: Optional[float] = None) -> bool:
        """Envoie une détection à l'API"""
        if not self.auth_token:
            logger.error("❌ Non authentifié - utilisez login() d'abord")
            return False
        
        detection_data = {
            "g_id": g_id,
            "object_type": object_type,
            "color": color,
        }
        
        if confidence is not None:
            detection_data["confidence"] = round(confidence, 3)
        
        try:
            response = self.session.post(
                f"{self.base_url}/detection",
                json=detection_data,
                timeout=10
            )
            
            if response.status_code == 200:
                data = response.json()
                if data.get("success"):
                    logger.info(f"🔍 Détection envoyée: {g_id} ({object_type}) - {color}")
                    return True
                else:
                    logger.error(f"❌ Échec envoi: {data.get('message')}")
            else:
                logger.error(f"❌ Erreur HTTP {response.status_code}: {response.text}")
                
        except requests.exceptions.RequestException as e:
            logger.error(f"❌ Erreur d'envoi: {e}")
            
        return False
    
    def get_stats(self) -> Optional[Dict[str, Any]]:
        """Récupère les statistiques actuelles"""
        if not self.auth_token:
            return None
            
        try:
            response = self.session.get(f"{self.base_url}/stats", timeout=10)
            
            if response.status_code == 200:
                data = response.json()
                if data.get("success"):
                    return data["data"]
                    
        except requests.exceptions.RequestException as e:
            logger.error(f"❌ Erreur récupération stats: {e}")
            
        return None

def generate_random_detection() -> Dict[str, Any]:
    """Génère une détection aléatoire pour simulation"""
    object_types = ["Carte microchip", "Carte personnalisée", "STM32"]
    colors = ["red", "green", "blue"]
    
    # Génération d'un G_ID unique
    timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
    random_id = random.randint(1000, 9999)
    
    object_type = random.choice(object_types)
    color = random.choice(colors)
    
    # Mapping des types pour G_ID
    type_prefixes = {
        "Carte microchip": "MCH",
        "Carte personnalisée": "CUS", 
        "STM32": "STM"
    }
    
    color_prefixes = {
        "red": "R",
        "green": "G", 
        "blue": "B"
    }
    
    g_id = f"{type_prefixes[object_type]}_{color_prefixes[color]}_{timestamp}_{random_id}"
    
    return {
        "g_id": g_id,
        "object_type": object_type,
        "color": color,
        "confidence": round(random.uniform(0.7, 0.99), 3)
    }

def simulate_detection_stream(sender: DetectionSender, count: int = 10, delay: float = 2.0):
    """Simule un flux de détections en temps réel"""
    logger.info(f"🚀 Démarrage de la simulation - {count} détections avec {delay}s d'intervalle")
    
    for i in range(count):
        detection = generate_random_detection()
        
        success = sender.send_detection(
            detection["g_id"],
            detection["object_type"],
            detection["color"],
            detection["confidence"]
        )
        
        if success:
            logger.info(f"✅ Détection {i+1}/{count} envoyée avec succès")
        else:
            logger.error(f"❌ Échec détection {i+1}/{count}")
        
        if i < count - 1:  # Pas de délai après la dernière détection
            logger.info(f"⏳ Attente {delay}s avant la prochaine détection...")
            time.sleep(delay)
    
    logger.info("🏁 Simulation terminée")

def send_predefined_detections(sender: DetectionSender):
    """Envoie un lot de détections prédéfinies pour les tests"""
    predefined = [
        {"g_id": "MCH_R_001", "object_type": "Carte microchip", "color": "red", "confidence": 0.95},
        {"g_id": "CUS_G_001", "object_type": "Carte personnalisée", "color": "green", "confidence": 0.88},
        {"g_id": "STM_B_001", "object_type": "STM32", "color": "blue", "confidence": 0.92},
        {"g_id": "MCH_R_002", "object_type": "Carte microchip", "color": "red", "confidence": 0.87},
        {"g_id": "CUS_B_001", "object_type": "Carte personnalisée", "color": "blue", "confidence": 0.91},
    ]
    
    logger.info(f"📦 Envoi de {len(predefined)} détections prédéfinies")
    
    for detection in predefined:
        success = sender.send_detection(**detection)
        if success:
            logger.info(f"✅ {detection['g_id']} envoyé")
        time.sleep(1)  # Petite pause entre les envois

def main():
    """Fonction principale"""
    print("🔍 Système d'Envoi de Détections IoT")
    print("=" * 50)
    
    sender = DetectionSender()
    
    # Tentative de connexion
    if not sender.login():
        logger.error("❌ Impossible de se connecter - Vérifiez que le serveur est démarré")
        return
    
    while True:
        print("\n📋 Options disponibles:")
        print("1. Envoyer une détection manuelle")
        print("2. Envoyer des détections prédéfinies")
        print("3. Simuler un flux de détections (10 détections)")
        print("4. Simulation longue (50 détections)")
        print("5. Afficher les statistiques")
        print("6. Test de performance (100 détections rapides)")
        print("0. Quitter")
        
        try:
            choice = input("\n👉 Votre choix: ").strip()
            
            if choice == "0":
                print("👋 Au revoir!")
                break
                
            elif choice == "1":
                print("\n📝 Détection manuelle:")
                g_id = input("G_ID: ").strip()
                
                print("Types disponibles:")
                print("1. Carte microchip")
                print("2. Carte personnalisée") 
                print("3. STM32")
                type_choice = input("Type (1-3): ").strip()
                
                object_types = {
                    "1": "Carte microchip",
                    "2": "Carte personnalisée",
                    "3": "STM32"
                }
                object_type = object_types.get(type_choice, "Carte microchip")
                
                print("Couleurs disponibles: red, green, blue")
                color = input("Couleur: ").strip()
                
                confidence_input = input("Confiance (0-1, optionnel): ").strip()
                confidence = float(confidence_input) if confidence_input else None
                
                sender.send_detection(g_id, object_type, color, confidence)
                
            elif choice == "2":
                send_predefined_detections(sender)
                
            elif choice == "3":
                simulate_detection_stream(sender, count=10, delay=2.0)
                
            elif choice == "4":
                simulate_detection_stream(sender, count=50, delay=1.0)
                
            elif choice == "5":
                stats = sender.get_stats()
                if stats:
                    print("\n📊 Statistiques actuelles:")
                    print(f"📈 Aujourd'hui: {stats.get('today', {})}")
                    print(f"📊 Total: {stats.get('total', {})}")
                    print(f"🕐 Dernière mise à jour: {stats.get('last_updated', 'N/A')}")
                else:
                    print("❌ Impossible de récupérer les statistiques")
                    
            elif choice == "6":
                logger.info("🚀 Test de performance - 100 détections rapides")
                start_time = time.time()
                simulate_detection_stream(sender, count=100, delay=0.1)
                end_time = time.time()
                logger.info(f"⚡ Test terminé en {end_time - start_time:.2f}s")
                
            else:
                print("❌ Choix invalide")
                
        except KeyboardInterrupt:
            print("\n🛑 Interruption par l'utilisateur")
            break
        except ValueError as e:
            print(f"❌ Erreur de saisie: {e}")
        except Exception as e:
            logger.error(f"❌ Erreur inattendue: {e}")

if __name__ == "__main__":
    main()