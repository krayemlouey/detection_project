#!/usr/bin/env python3
"""
Système de détection d'objets par couleur en temps réel
Intégré avec l'API backend Rust pour le stockage des détections
"""

import cv2
import numpy as np
import requests
import json
import time
import threading
from datetime import datetime
import argparse
import sys
import os

# Configuration de l'API
API_BASE_URL = "http://localhost:3000/api"
API_ENDPOINT = f"{API_BASE_URL}/detections"

# Configuration des couleurs HSV (Hue, Saturation, Value)
COLORS = {
    'red': {
        'ranges': [
            (np.array([0, 120, 70]), np.array([10, 255, 255])),    # Rouge bas
            (np.array([170, 120, 70]), np.array([180, 255, 255]))  # Rouge haut
        ],
        'object_type': 'Carte microchip',
        'display_color': (0, 0, 255)  # BGR pour OpenCV
    },
    'green': {
        'ranges': [
            (np.array([36, 50, 70]), np.array([89, 255, 255]))
        ],
        'object_type': 'Carte personnalisée',
        'display_color': (0, 255, 0)
    },
    'blue': {
        'ranges': [
            (np.array([90, 50, 70]), np.array([128, 255, 255]))
        ],
        'object_type': 'STM32',
        'display_color': (255, 0, 0)
    }
}

class DetectionSystem:
    def __init__(self, camera_index=0, api_url=API_ENDPOINT):
        self.camera_index = camera_index
        self.api_url = api_url
        self.cap = None
        self.running = False
        self.detection_enabled = True
        self.detection_interval = 1.0  # Secondes entre détections
        self.last_detection_time = 0
        
        # Compteurs
        self.counters = {color: 0 for color in COLORS.keys()}
        self.total_detections = 0
        self.session_start = datetime.now()
        
        # Configuration de la capture
        self.frame_width = 640
        self.frame_height = 480
        self.detection_area_size = 20  # Taille des blocs de détection
        
        # Historique des détections récentes (pour éviter les doublons)
        self.recent_detections = {}
        self.detection_cooldown = 2.0  # Secondes avant de pouvoir re-détecter le même objet

    def init_camera(self):
        """Initialiser la caméra"""
        try:
            self.cap = cv2.VideoCapture(self.camera_index)
            if not self.cap.isOpened():
                print(f"❌ Impossible d'ouvrir la caméra {self.camera_index}")
                return False
            
            # Configuration de la caméra
            self.cap.set(cv2.CAP_PROP_FRAME_WIDTH, self.frame_width)
            self.cap.set(cv2.CAP_PROP_FRAME_HEIGHT, self.frame_height)
            self.cap.set(cv2.CAP_PROP_FPS, 30)
            
            print(f"✅ Caméra initialisée: {self.frame_width}x{self.frame_height}")
            return True
        except Exception as e:
            print(f"❌ Erreur lors de l'initialisation de la caméra: {e}")
            return False

    def send_detection_to_api(self, color, object_type):
        """Envoyer une détection à l'API backend"""
        try:
            timestamp = int(time.time())
            g_id = f"{color.upper()}_{object_type.replace(' ', '_').upper()}_{timestamp}"
            
            payload = {
                "g_id": g_id,
                "object_type": object_type,
                "color": color
            }
            
            response = requests.post(
                self.api_url,
                json=payload,
                headers={'Content-Type': 'application/json'},
                timeout=5
            )
            
            if response.status_code == 200:
                data = response.json()
                if data.get('success'):
                    detection_data = data.get('data', {})
                    ref_count = detection_data.get('ref_count', 1)
                    print(f"✅ API: {color} {object_type} enregistré (ref: {ref_count})")
                    return True
                else:
                    print(f"⚠️ API: {data.get('message', 'Erreur inconnue')}")
            else:
                print(f"❌ API HTTP {response.status_code}: {response.text}")
                
        except requests.exceptions.RequestException as e:
            print(f"🌐 Erreur de connexion API: {e}")
        except Exception as e:
            print(f"❌ Erreur lors de l'envoi à l'API: {e}")
        
        return False

    def detect_color_objects(self, frame):
        """Détecter les objets colorés dans l'image"""
        hsv = cv2.cvtColor(frame, cv2.COLOR_BGR2HSV)
        detections = []
        
        for color_name, color_config in COLORS.items():
            # Créer un masque combiné pour toutes les plages de couleur
            combined_mask = np.zeros(hsv.shape[:2], dtype=np.uint8)
            
            for lower, upper in color_config['ranges']:
                mask = cv2.inRange(hsv, lower, upper)
                combined_mask = cv2.bitwise_or(combined_mask, mask)
            
            # Nettoyer le masque avec des opérations morphologiques
            kernel = np.ones((5, 5), np.uint8)
            combined_mask = cv2.morphologyEx(combined_mask, cv2.MORPH_OPEN, kernel)
            combined_mask = cv2.morphologyEx(combined_mask, cv2.MORPH_CLOSE, kernel)
            
            # Trouver les contours
            contours, _ = cv2.findContours(combined_mask, cv2.RETR_EXTERNAL, cv2.CHAIN_APPROX_SIMPLE)
            
            for contour in contours:
                area = cv2.contourArea(contour)
                if area > 500:  # Filtrer les petits objets
                    # Calculer le rectangle englobant
                    x, y, w, h = cv2.boundingRect(contour)
                    
                    # Vérifier si on a déjà détecté cet objet récemment
                    detection_key = f"{color_name}_{x//50}_{y//50}"  # Grille grossière
                    current_time = time.time()
                    
                    if (detection_key not in self.recent_detections or 
                        current_time - self.recent_detections[detection_key] > self.detection_cooldown):
                        
                        detections.append({
                            'color': color_name,
                            'object_type': color_config['object_type'],
                            'bbox': (x, y, w, h),
                            'area': area,
                            'display_color': color_config['display_color']
                        })
                        
                        self.recent_detections[detection_key] = current_time
        
        return detections

    def process_detections(self, detections):
        """Traiter les détections trouvées"""
        current_time = time.time()
        
        for detection in detections:
            color = detection['color']
            object_type = detection['object_type']
            
            # Mettre à jour les compteurs
            self.counters[color] += 1
            self.total_detections += 1
            
            # Envoyer à l'API si activé
            if self.detection_enabled:
                threading.Thread(
                    target=self.send_detection_to_api,
                    args=(color, object_type),
                    daemon=True
                ).start()
            
            print(f"🔍 Détecté: {color} {object_type} (Total: {self.total_detections})")

    def draw_detections(self, frame, detections):
        """Dessiner les détections sur l'image"""
        for detection in detections:
            x, y, w, h = detection['bbox']
            color = detection['display_color']
            
            # Rectangle de détection
            cv2.rectangle(frame, (x, y), (x + w, y + h), color, 2)
            
            # Label
            label = f"{detection['color']} {detection['object_type']}"
            label_size = cv2.getTextSize(label, cv2.FONT_HERSHEY_SIMPLEX, 0.6, 2)[0]
            
            # Background du label
            cv2.rectangle(frame, (x, y - label_size[1] - 10), 
                         (x + label_size[0], y), color, -1)
            
            # Texte du label
            cv2.putText(frame, label, (x, y - 5), 
                       cv2.FONT_HERSHEY_SIMPLEX, 0.6, (255, 255, 255), 2)

    def draw_ui(self, frame):
        """Dessiner l'interface utilisateur sur l'image"""
        h, w = frame.shape[:2]
        
        # Zone d'informations (fond semi-transparent)
        overlay = frame.copy()
        cv2.rectangle(overlay, (0, 0), (w, 120), (0, 0, 0), -1)
        cv2.addWeighted(overlay, 0.7, frame, 0.3, 0, frame)
        
        # Titre
        cv2.putText(frame, "Systeme de Detection IoT", (10, 25), 
                   cv2.FONT_HERSHEY_SIMPLEX, 0.8, (0, 255, 255), 2)
        
        # Compteurs
        y_pos = 50
        for i, (color, count) in enumerate(self.counters.items()):
            color_info = COLORS[color]
            text = f"{color.upper()}: {count} ({color_info['object_type']})"
            cv2.putText(frame, text, (10, y_pos + i * 20), 
                       cv2.FONT_HERSHEY_SIMPLEX, 0.5, color_info['display_color'], 1)
        
        # Informations de session
        session_duration = datetime.now() - self.session_start
        duration_str = str(session_duration).split('.')[0]  # Enlever les microsecondes
        
        info_text = f"Total: {self.total_detections} | Duree: {duration_str}"
        cv2.putText(frame, info_text, (w - 300, 25), 
                   cv2.FONT_HERSHEY_SIMPLEX, 0.5, (255, 255, 255), 1)
        
        # État de détection
        status = "ACTIF" if self.detection_enabled else "PAUSE"
        status_color = (0, 255, 0) if self.detection_enabled else (0, 255, 255)
        cv2.putText(frame, f"Detection: {status}", (w - 150, 45), 
                   cv2.FONT_HERSHEY_SIMPLEX, 0.5, status_color, 1)
        
        # Aide (contrôles)
        help_text = "S:Start Q:Quit C:Capture R:Reset"
        cv2.putText(frame, help_text, (10, h - 10), 
                   cv2.FONT_HERSHEY_SIMPLEX, 0.4, (200, 200, 200), 1)

    def reset_counters(self):
        """Remettre à zéro tous les compteurs"""
        self.counters = {color: 0 for color in COLORS.keys()}
        self.total_detections = 0
        self.session_start = datetime.now()
        self.recent_detections.clear()
        print("🔄 Compteurs remis à zéro")

    def save_screenshot(self, frame):
        """Sauvegarder une capture d'écran"""
        timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
        filename = f"detection_capture_{timestamp}.jpg"
        cv2.imwrite(filename, frame)
        print(f"📸 Capture sauvegardée: {filename}")

    def run(self):
        """Boucle principale de détection"""
        if not self.init_camera():
            return False
        
        print("🚀 Système de détection démarré")
        print("Contrôles: S=Start/Pause, Q=Quitter, C=Capturer, R=Reset")
        print("-" * 50)
        
        self.running = True
        
        try:
            while self.running:
                ret, frame = self.cap.read()
                if not ret:
                    print("❌ Impossible de lire l'image de la caméra")
                    break
                
                # Traitement des détections
                current_time = time.time()
                if (self.detection_enabled and 
                    current_time - self.last_detection_time > self.detection_interval):
                    
                    detections = self.detect_color_objects(frame)
                    if detections:
                        self.process_detections(detections)
                        self.draw_detections(frame, detections)
                    
                    self.last_detection_time = current_time
                
                # Interface utilisateur
                self.draw_ui(frame)
                
                # Afficher l'image
                cv2.imshow('Detection System', frame)
                
                # Gestion des touches
                key = cv2.waitKey(1) & 0xFF
                
                if key == ord('q') or key == 27:  # Q ou Escape
                    print("👋 Arrêt du système...")
                    break
                elif key == ord('s'):  # Start/Stop
                    self.detection_enabled = not self.detection_enabled
                    status = "activée" if self.detection_enabled else "désactivée"
                    print(f"🔄 Détection {status}")
                elif key == ord('c'):  # Capture
                    self.save_screenshot(frame)
                elif key == ord('r'):  # Reset
                    self.reset_counters()
                elif key == ord('h'):  # Help
                    self.print_help()
                
        except KeyboardInterrupt:
            print("\n⚠️ Interruption clavier détectée")
        except Exception as e:
            print(f"❌ Erreur durant l'exécution: {e}")
        finally:
            self.cleanup()
        
        return True

    def cleanup(self):
        """Nettoyer les ressources"""
        self.running = False
        if self.cap:
            self.cap.release()
        cv2.destroyAllWindows()
        
        # Afficher le résumé de la session
        print("\n" + "="*50)
        print("📊 RESUME DE LA SESSION")
        print("="*50)
        session_duration = datetime.now() - self.session_start
        print(f"Durée: {session_duration}")
        print(f"Total détections: {self.total_detections}")
        
        for color, count in self.counters.items():
            if count > 0:
                print(f"  {color.upper()}: {count} ({COLORS[color]['object_type']})")
        print("="*50)

    def print_help(self):
        """Afficher l'aide"""
        print("\n" + "="*40)
        print("🆘 AIDE - CONTROLES")
        print("="*40)
        print("S : Activer/Désactiver la détection")
        print("Q : Quitter le programme")
        print("C : Capturer une image")
        print("R : Remettre à zéro les compteurs")
        print("H : Afficher cette aide")
        print("ESC : Quitter")
        print("="*40)

    def test_api_connection(self):
        """Tester la connexion à l'API"""
        try:
            health_url = f"{API_BASE_URL}/health"
            response = requests.get(health_url, timeout=5)
            if response.status_code == 200:
                print("✅ API backend accessible")
                return True
            else:
                print(f"⚠️ API répond avec le code: {response.status_code}")
        except requests.exceptions.RequestException as e:
            print(f"❌ API backend inaccessible: {e}")
            print("💡 Assurez-vous que le serveur Rust est démarré (cargo run)")
        
        return False

    def adjust_detection_sensitivity(self, sensitivity_level):
        """Ajuster la sensibilité de détection"""
        if sensitivity_level == "low":
            self.detection_interval = 2.0
            self.detection_cooldown = 3.0
        elif sensitivity_level == "medium":
            self.detection_interval = 1.0
            self.detection_cooldown = 2.0
        elif sensitivity_level == "high":
            self.detection_interval = 0.5
            self.detection_cooldown = 1.0
        
        print(f"🎛️ Sensibilité réglée sur: {sensitivity_level}")

def main():
    """Fonction principale"""
    parser = argparse.ArgumentParser(description='Système de détection d\'objets par couleur')
    parser.add_argument('--camera', '-c', type=int, default=0, 
                       help='Index de la caméra (défaut: 0)')
    parser.add_argument('--api-url', '-a', type=str, default=API_ENDPOINT,
                       help=f'URL de l\'API (défaut: {API_ENDPOINT})')
    parser.add_argument('--sensitivity', '-s', choices=['low', 'medium', 'high'], 
                       default='medium', help='Niveau de sensibilité (défaut: medium)')
    parser.add_argument('--no-api', action='store_true', 
                       help='Désactiver l\'envoi vers l\'API')
    parser.add_argument('--test-api', action='store_true',
                       help='Tester la connexion à l\'API et quitter')
    
    args = parser.parse_args()
    
    # Affichage de démarrage
    print("🎯 SYSTEME DE DETECTION D'OBJETS IoT")
    print("="*50)
    print(f"Caméra: {args.camera}")
    print(f"API: {args.api_url}")
    print(f"Sensibilité: {args.sensitivity}")
    print("="*50)
    
    # Test de l'API si demandé
    if args.test_api:
        system = DetectionSystem(args.camera, args.api_url)
        if system.test_api_connection():
            print("✅ Test API réussi")
            return 0
        else:
            print("❌ Test API échoué")
            return 1
    
    # Créer le système de détection
    system = DetectionSystem(args.camera, args.api_url)
    
    # Configurer la sensibilité
    system.adjust_detection_sensitivity(args.sensitivity)
    
    # Désactiver l'API si demandé
    if args.no_api:
        system.detection_enabled = False
        print("⚠️ Mode hors-ligne activé (pas d'envoi API)")
    else:
        # Tester la connexion API
        system.test_api_connection()
    
    # Lancer le système
    try:
        if system.run():
            print("✅ Session terminée avec succès")
            return 0
        else:
            print("❌ Erreur durant l'exécution")
            return 1
    except KeyboardInterrupt:
        print("\n👋 Arrêt par l'utilisateur")
        return 0

# Classes utilitaires pour les tests et configuration avancée
class ColorCalibrator:
    """Utilitaire pour calibrer les couleurs"""
    
    def __init__(self):
        self.trackbars_created = False
    
    def create_trackbars(self):
        """Créer les barres de réglage HSV"""
        cv2.namedWindow('HSV Calibrator')
        cv2.createTrackbar('H Min', 'HSV Calibrator', 0, 179, lambda x: None)
        cv2.createTrackbar('H Max', 'HSV Calibrator', 179, 179, lambda x: None)
        cv2.createTrackbar('S Min', 'HSV Calibrator', 0, 255, lambda x: None)
        cv2.createTrackbar('S Max', 'HSV Calibrator', 255, 255, lambda x: None)
        cv2.createTrackbar('V Min', 'HSV Calibrator', 0, 255, lambda x: None)
        cv2.createTrackbar('V Max', 'HSV Calibrator', 255, 255, lambda x: None)
        self.trackbars_created = True
    
    def get_hsv_range(self):
        """Obtenir les valeurs HSV des trackbars"""
        if not self.trackbars_created:
            return None
        
        h_min = cv2.getTrackbarPos('H Min', 'HSV Calibrator')
        h_max = cv2.getTrackbarPos('H Max', 'HSV Calibrator')
        s_min = cv2.getTrackbarPos('S Min', 'HSV Calibrator')
        s_max = cv2.getTrackbarPos('S Max', 'HSV Calibrator')
        v_min = cv2.getTrackbarPos('V Min', 'HSV Calibrator')
        v_max = cv2.getTrackbarPos('V Max', 'HSV Calibrator')
        
        return (np.array([h_min, s_min, v_min]), np.array([h_max, s_max, v_max]))

class PerformanceMonitor:
    """Moniteur de performance pour le système de détection"""
    
    def __init__(self):
        self.frame_times = []
        self.detection_times = []
        self.api_response_times = []
        self.start_time = time.time()
    
    def record_frame_time(self, frame_time):
        """Enregistrer le temps de traitement d'une frame"""
        self.frame_times.append(frame_time)
        if len(self.frame_times) > 100:  # Garder seulement les 100 dernières
            self.frame_times.pop(0)
    
    def record_detection_time(self, detection_time):
        """Enregistrer le temps de détection"""
        self.detection_times.append(detection_time)
        if len(self.detection_times) > 100:
            self.detection_times.pop(0)
    
    def get_fps(self):
        """Calculer les FPS moyens"""
        if not self.frame_times:
            return 0
        avg_frame_time = sum(self.frame_times) / len(self.frame_times)
        return 1.0 / avg_frame_time if avg_frame_time > 0 else 0
    
    def get_stats(self):
        """Obtenir les statistiques de performance"""
        return {
            'fps': self.get_fps(),
            'avg_detection_time': sum(self.detection_times) / len(self.detection_times) if self.detection_times else 0,
            'total_runtime': time.time() - self.start_time,
            'total_frames': len(self.frame_times)
        }

if __name__ == "__main__":
    # Vérifier les dépendances
    try:
        import cv2
        import numpy as np
        import requests
    except ImportError as e:
        print(f"❌ Dépendance manquante: {e}")
        print("💡 Installez les dépendances: pip install -r requirements.txt")
        sys.exit(1)
    
    # Lancer le programme principal
    sys.exit(main())