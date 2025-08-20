import cv2
import numpy as np
import requests
import json
import time
from datetime import datetime
import base64
import os

class ColorObjectDetector:
    def __init__(self):
        self.cap = None
        self.running = False
        
        # Configuration des couleurs HSV (améliorée)
        self.colors = {
            'red': [
                (0, 120, 70), (10, 255, 255),      # Rouge bas
                (170, 120, 70), (180, 255, 255)    # Rouge haut
            ],
            'green': [(35, 50, 50), (85, 255, 255)],    # Vert
            'blue': [(90, 50, 50), (130, 255, 255)]     # Bleu
        }
        
        # Mapping des couleurs vers les types d'objets
        self.color_to_type = {
            'red': 'Carte microchip',
            'green': 'Carte personnalisée', 
            'blue': 'STM32'
        }
        
        # Configuration
        self.min_area = 500  # Surface minimum pour détection
        self.api_url = "http://localhost:3000/api/detections"
        self.detection_cooldown = {}  # Pour éviter les détections en double
        self.cooldown_time = 3.0  # 3 secondes entre détections
        
        # Compteurs
        self.frame_count = 0
        self.detection_count = {'red': 0, 'green': 0, 'blue': 0}
        
        # Dossier pour sauvegarder les captures
        self.captures_dir = "captures"
        if not os.path.exists(self.captures_dir):
            os.makedirs(self.captures_dir)
    
    def initialize_camera(self):
        """Initialise la caméra"""
        try:
            # Essayer différents indices de caméra
            for i in range(3):
                self.cap = cv2.VideoCapture(i)
                if self.cap.isOpened():
                    print(f"✅ Caméra {i} connectée avec succès")
                    # Configuration de la caméra
                    self.cap.set(cv2.CAP_PROP_FRAME_WIDTH, 640)
                    self.cap.set(cv2.CAP_PROP_FRAME_HEIGHT, 480)
                    self.cap.set(cv2.CAP_PROP_FPS, 30)
                    return True
            
            print("❌ Aucune caméra trouvée")
            return False
            
        except Exception as e:
            print(f"❌ Erreur d'initialisation caméra: {e}")
            return False
    
    def detect_color_objects(self, frame):
        """Détecte les objets colorés dans le frame"""
        detections = []
        
        # Convertir en HSV
        hsv = cv2.cvtColor(frame, cv2.COLOR_BGR2HSV)
        
        # Appliquer un flou pour réduire le bruit
        hsv = cv2.GaussianBlur(hsv, (5, 5), 0)
        
        for color_name, ranges in self.colors.items():
            mask = np.zeros(hsv.shape[:2], dtype=np.uint8)
            
            # Gérer les plages multiples (comme pour le rouge)
            if len(ranges) == 4:  # Rouge avec deux plages
                mask1 = cv2.inRange(hsv, ranges[0], ranges[1])
                mask2 = cv2.inRange(hsv, ranges[2], ranges[3])
                mask = cv2.bitwise_or(mask1, mask2)
            else:
                mask = cv2.inRange(hsv, ranges[0], ranges[1])
            
            # Opérations morphologiques pour nettoyer le masque
            kernel = np.ones((5, 5), np.uint8)
            mask = cv2.morphologyEx(mask, cv2.MORPH_CLOSE, kernel)
            mask = cv2.morphologyEx(mask, cv2.MORPH_OPEN, kernel)
            
            # Trouver les contours
            contours, _ = cv2.findContours(mask, cv2.RETR_EXTERNAL, cv2.CHAIN_APPROX_SIMPLE)
            
            for contour in contours:
                area = cv2.contourArea(contour)
                
                if area > self.min_area:
                    # Calculer le centre et les dimensions
                    M = cv2.moments(contour)
                    if M["m00"] != 0:
                        cx = int(M["m10"] / M["m00"])
                        cy = int(M["m01"] / M["m00"])
                        
                        # Rectangle englobant
                        x, y, w, h = cv2.boundingRect(contour)
                        
                        # Calculer le ratio et la solidité pour filtrer
                        aspect_ratio = w / float(h)
                        hull = cv2.convexHull(contour)
                        hull_area = cv2.contourArea(hull)
                        solidity = area / hull_area if hull_area > 0 else 0
                        
                        detection = {
                            'color': color_name,
                            'type': self.color_to_type[color_name],
                            'center': (cx, cy),
                            'bbox': (x, y, w, h),
                            'area': area,
                            'aspect_ratio': aspect_ratio,
                            'solidity': solidity,
                            'contour': contour
                        }
                        
                        detections.append(detection)
        
        return detections
    
    def draw_detections(self, frame, detections):
        """Dessine les détections sur le frame"""
        for detection in detections:
            color = detection['color']
            x, y, w, h = detection['bbox']
            cx, cy = detection['center']
            
            # Couleur pour le dessin
            draw_color = {
                'red': (0, 0, 255),
                'green': (0, 255, 0),
                'blue': (255, 0, 0)
            }[color]
            
            # Dessiner le contour
            cv2.drawContours(frame, [detection['contour']], -1, draw_color, 2)
            
            # Dessiner le rectangle
            cv2.rectangle(frame, (x, y), (x + w, y + h), draw_color, 2)
            
            # Dessiner le centre
            cv2.circle(frame, (cx, cy), 5, draw_color, -1)
            
            # Ajouter le texte
            text = f"{detection['type']} ({color.upper()})"
            cv2.putText(frame, text, (x, y - 10), cv2.FONT_HERSHEY_SIMPLEX, 0.6, draw_color, 2)
            
            # Informations supplémentaires
            info_text = f"Area: {int(detection['area'])}"
            cv2.putText(frame, info_text, (x, y + h + 20), cv2.FONT_HERSHEY_SIMPLEX, 0.4, draw_color, 1)
    
    def save_detection_image(self, frame, detection):
        """Sauvegarde l'image de la détection"""
        timestamp = int(time.time())
        filename = f"capture_{detection['color']}_{timestamp}.jpg"
        filepath = os.path.join(self.captures_dir, filename)
        
        # Extraire la région d'intérêt
        x, y, w, h = detection['bbox']
        # Ajouter une marge
        margin = 20
        x1 = max(0, x - margin)
        y1 = max(0, y - margin)
        x2 = min(frame.shape[1], x + w + margin)
        y2 = min(frame.shape[0], y + h + margin)
        
        roi = frame[y1:y2, x1:x2]
        
        success = cv2.imwrite(filepath, roi)
        if success:
            print(f"📸 Image sauvegardée: {filename}")
            return filename
        return None
    
    def send_to_api(self, detection, image_filename=None):
        """Envoie la détection à l'API"""
        timestamp = datetime.now().isoformat()
        
        data = {
            "g_id": f"{detection['color'].upper()}_{detection['type'].replace(' ', '_')}_{int(time.time())}",
            "object_type": detection['type'],
            "color": detection['color'],
            "datetime": timestamp,
            "area": int(detection['area']),
            "center_x": detection['center'][0],
            "center_y": detection['center'][1],
            "image_filename": image_filename
        }
        
        try:
            response = requests.post(self.api_url, 
                                   json=data, 
                                   timeout=5,
                                   headers={'Content-Type': 'application/json'})
            
            if response.status_code == 200:
                print(f"✅ Détection envoyée: {detection['type']} ({detection['color']})")
                return True
            else:
                print(f"❌ Erreur API ({response.status_code}): {response.text}")
                return False
                
        except requests.RequestException as e:
            print(f"❌ Erreur de connexion API: {e}")
            return False
    
    def is_duplicate_detection(self, detection):
        """Vérifie si c'est une détection en double"""
        color = detection['color']
        current_time = time.time()
        
        # Vérifier le cooldown
        if color in self.detection_cooldown:
            if current_time - self.detection_cooldown[color] < self.cooldown_time:
                return True
        
        # Mettre à jour le timestamp
        self.detection_cooldown[color] = current_time
        return False
    
    def display_stats(self, frame):
        """Affiche les statistiques sur le frame"""
        # Background pour les stats
        overlay = frame.copy()
        cv2.rectangle(overlay, (10, 10), (300, 120), (0, 0, 0), -1)
        cv2.addWeighted(overlay, 0.7, frame, 0.3, 0, frame)
        
        # Texte des statistiques
        stats_text = [
            f"Frame: {self.frame_count}",
            f"Rouge: {self.detection_count['red']}",
            f"Vert: {self.detection_count['green']}",
            f"Bleu: {self.detection_count['blue']}",
            f"Total: {sum(self.detection_count.values())}"
        ]
        
        for i, text in enumerate(stats_text):
            cv2.putText(frame, text, (20, 35 + i * 20), 
                       cv2.FONT_HERSHEY_SIMPLEX, 0.5, (255, 255, 255), 1)
    
    def run(self):
        """Lance la détection en temps réel"""
        if not self.initialize_camera():
            return
        
        print("🎯 Détection démarrée - Appuyez sur 'q' pour quitter")
        print("📋 Commandes:")
        print("  's' - Sauvegarder le frame actuel")
        print("  'r' - Reset des compteurs")
        print("  'c' - Capturer et sauvegarder toutes les détections")
        print("  'q' - Quitter")
        
        self.running = True
        
        try:
            while self.running:
                ret, frame = self.cap.read()
                if not ret:
                    print("❌ Impossible de lire le frame de la caméra")
                    break
                
                self.frame_count += 1
                
                # Détection des objets
                detections = self.detect_color_objects(frame)
                
                # Traiter chaque détection
                for detection in detections:
                    if not self.is_duplicate_detection(detection):
                        self.detection_count[detection['color']] += 1
                        
                        # Sauvegarder l'image automatiquement lors d'une détection
                        image_filename = self.save_detection_image(frame, detection)
                        
                        # Envoyer à l'API
                        self.send_to_api(detection, image_filename)
                
                # Dessiner les détections
                self.draw_detections(frame, detections)
                
                # Afficher les statistiques
                self.display_stats(frame)
                
                # Afficher le frame
                cv2.imshow('Détection d\'Objets Colorés', frame)
                
                # Gestion des touches
                key = cv2.waitKey(1) & 0xFF
                if key == ord('q'):
                    self.running = False
                elif key == ord('s'):
                    self.save_current_frame(frame)
                elif key == ord('r'):
                    self.reset_counters()
                elif key == ord('c'):
                    self.capture_all_detections(frame, detections)
        
        except KeyboardInterrupt:
            print("\n⏹️ Arrêt demandé par l'utilisateur")
        
        finally:
            self.cleanup()
    
    def save_current_frame(self, frame):
        """Sauvegarde le frame actuel"""
        timestamp = int(time.time())
        filename = f"frame_{timestamp}.jpg"
        filepath = os.path.join(self.captures_dir, filename)
        
        success = cv2.imwrite(filepath, frame)
        if success:
            print(f"📸 Frame sauvegardé: {filename}")
        else:
            print("❌ Erreur lors de la sauvegarde")
    
    def reset_counters(self):
        """Remet les compteurs à zéro"""
        self.detection_count = {'red': 0, 'green': 0, 'blue': 0}
        self.frame_count = 0
        self.detection_cooldown = {}
        print("🔄 Compteurs remis à zéro")
    
    def capture_all_detections(self, frame, detections):
        """Capture toutes les détections actuelles"""
        if not detections:
            print("ℹ️ Aucune détection à capturer")
            return
        
        for i, detection in enumerate(detections):
            image_filename = self.save_detection_image(frame, detection)
            if image_filename:
                self.send_to_api(detection, image_filename)
        
        print(f"📸 {len(detections)} détection(s) capturée(s)")
    
    def cleanup(self):
        """Nettoie les ressources"""
        if self.cap:
            self.cap.release()
        cv2.destroyAllWindows()
        print("🧹 Ressources nettoyées")
    
    def test_colors(self):
        """Mode test pour calibrer les couleurs"""
        if not self.initialize_camera():
            return
        
        print("🔧 Mode test de couleurs - Appuyez sur 'q' pour quitter")
        
        while True:
            ret, frame = self.cap.read()
            if not ret:
                break
            
            # Convertir en HSV
            hsv = cv2.cvtColor(frame, cv2.COLOR_BGR2HSV)
            
            # Afficher les valeurs HSV au centre
            h, w = frame.shape[:2]
            center_hsv = hsv[h//2, w//2]
            
            # Dessiner le point central
            cv2.circle(frame, (w//2, h//2), 10, (0, 255, 0), 2)
            cv2.putText(frame, f"HSV: {center_hsv}", (20, 30), 
                       cv2.FONT_HERSHEY_SIMPLEX, 0.7, (255, 255, 255), 2)
            
            # Tester chaque couleur
            for i, (color_name, ranges) in enumerate(self.colors.items()):
                mask = np.zeros(hsv.shape[:2], dtype=np.uint8)
                
                if len(ranges) == 4:  # Rouge
                    mask1 = cv2.inRange(hsv, ranges[0], ranges[1])
                    mask2 = cv2.inRange(hsv, ranges[2], ranges[3])
                    mask = cv2.bitwise_or(mask1, mask2)
                else:
                    mask = cv2.inRange(hsv, ranges[0], ranges[1])
                
                # Afficher le masque dans une petite fenêtre
                small_mask = cv2.resize(mask, (150, 100))
                cv2.imshow(f'Masque {color_name}', small_mask)
            
            cv2.imshow('Test Couleurs', frame)
            
            if cv2.waitKey(1) & 0xFF == ord('q'):
                break
        
        self.cleanup()

def main():
    """Fonction principale"""
    detector = ColorObjectDetector()
    
    import sys
    if len(sys.argv) > 1 and sys.argv[1] == '--test':
        detector.test_colors()
    else:
        detector.run()

if __name__ == "__main__":
    main()